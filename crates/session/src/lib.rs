//! One live editing session shared by the desktop and external adapters.
pub mod effects;
pub mod gateway;
#[cfg(unix)]
pub mod ipc;
use blyacut_core::{id, Asset, Edit, Editor, Project, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub project: Project,
    pub revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RenderJob {
    pub id: String,
    pub revision: u64,
    pub progress: f64,
    pub status: String,
    pub error: Option<String>,
    /// Output path, so agents can find the file they asked for.
    #[serde(default)]
    pub output: Option<std::path::PathBuf>,
    /// Measured after a successful export.
    #[serde(default)]
    pub loudness: Option<blyacut_engine::studio::Loudness>,
    /// Timeline notes at start plus level problems found after export.
    #[serde(default)]
    pub warnings: Vec<String>,
}
fn stale(expected: u64, current: u64) -> String {
    format!(
        "Stale revision: expected {expected}, current {current}; re-read get_project and re-plan"
    )
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub revision: u64,
    pub edits: Vec<Edit>,
    pub preview: Project,
}
#[derive(Clone, Debug)]
pub enum Event {
    Project(Snapshot),
    Render(RenderJob),
    Proposal(Option<Proposal>),
}
pub struct Session {
    editor: Mutex<Editor>,
    pending: Mutex<Option<Proposal>>,
    job: Mutex<Option<(RenderJob, Arc<AtomicBool>)>>,
    emit: Box<dyn Fn(Event) + Send + Sync>,
}
impl Default for Session {
    fn default() -> Self {
        Self::new(|_| {})
    }
}
impl Session {
    pub fn new(emit: impl Fn(Event) + Send + Sync + 'static) -> Self {
        Self {
            editor: Mutex::new(Editor::default()),
            pending: Mutex::new(None),
            job: Mutex::new(None),
            emit: Box::new(emit),
        }
    }
    fn editor(&self) -> Result<std::sync::MutexGuard<'_, Editor>> {
        self.editor
            .lock()
            .map_err(|_| "Editor lock poisoned".into())
    }
    fn snapshot_of(e: &Editor) -> Snapshot {
        Snapshot {
            project: e.project.clone(),
            revision: e.revision,
        }
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        let editor = self.editor()?;
        Ok(Self::snapshot_of(&editor))
    }
    fn changed(&self, snapshot: Snapshot) -> Snapshot {
        if let Ok(mut pending) = self.pending.lock() {
            if pending
                .as_ref()
                .is_some_and(|p| p.revision < snapshot.revision)
            {
                *pending = None;
                (self.emit)(Event::Proposal(None));
            }
        }
        (self.emit)(Event::Project(snapshot.clone()));
        snapshot
    }
    pub fn edit(&self, edit: Edit) -> Result<Snapshot> {
        let rev = self.snapshot()?.revision;
        self.apply(&[edit], rev)
    }
    pub fn apply(&self, edits: &[Edit], expected_revision: u64) -> Result<Snapshot> {
        let snapshot = {
            let mut e = self.editor()?;
            e.batch(edits, expected_revision)?;
            Self::snapshot_of(&e)
        };
        Ok(self.changed(snapshot))
    }
    pub fn import(&self, asset: Asset) -> Result<Snapshot> {
        let revision = self.snapshot()?.revision;
        self.import_at(asset, revision)
    }
    pub fn import_at(&self, asset: Asset, revision: u64) -> Result<Snapshot> {
        let snapshot = {
            let mut e = self.editor()?;
            if e.revision != revision {
                return Err("Project changed during import; retry".into());
            }
            e.import(asset)?;
            Self::snapshot_of(&e)
        };
        Ok(self.changed(snapshot))
    }
    pub fn replace(&self, project: Project) -> Result<Snapshot> {
        let snapshot = {
            let mut e = self.editor()?;
            e.replace(project)?;
            Self::snapshot_of(&e)
        };
        Ok(self.changed(snapshot))
    }
    pub fn history(&self, redo: bool, expected_revision: Option<u64>) -> Result<Snapshot> {
        let snapshot = {
            let mut e = self.editor()?;
            if let Some(r) = expected_revision.filter(|r| *r != e.revision) {
                return Err(stale(r, e.revision));
            }
            if redo {
                e.redo()
            } else {
                e.undo()
            };
            Self::snapshot_of(&e)
        };
        Ok(self.changed(snapshot))
    }
    pub fn propose(&self, edits: Vec<Edit>, revision: u64) -> Result<Proposal> {
        let proposal = {
            let e = self.editor()?;
            if e.revision != revision {
                return Err(stale(revision, e.revision));
            }
            let preview = e.preview(&edits)?;
            let proposal = Proposal {
                id: id(),
                revision,
                edits,
                preview,
            };
            *self.pending.lock().map_err(|e| e.to_string())? = Some(proposal.clone());
            proposal
        };
        (self.emit)(Event::Proposal(Some(proposal.clone())));
        Ok(proposal)
    }
    pub fn pending(&self) -> Result<Option<Proposal>> {
        Ok(self.pending.lock().map_err(|e| e.to_string())?.clone())
    }
    /// Only the desktop exposes this method. Agents cannot approve their own proposal.
    pub fn resolve_proposal(&self, proposal_id: &str, accept: bool) -> Result<Snapshot> {
        let snapshot = {
            let mut e = self.editor()?;
            let mut pending = self.pending.lock().map_err(|e| e.to_string())?;
            let p = pending.as_ref().ok_or("No pending proposal")?;
            if p.id != proposal_id {
                return Err("Proposal ID mismatch".into());
            }
            if accept {
                e.batch(&p.edits, p.revision)?;
            }
            *pending = None;
            Self::snapshot_of(&e)
        };
        (self.emit)(Event::Proposal(None));
        Ok(self.changed(snapshot))
    }
    /// Hold the main-track frame at `at_ms` for `duration_ms`: split there and insert a still
    /// (written to `png`) carrying the clip's effects. Undo takes two steps (the edit, then the
    /// still's import). Returns the still's clip ID.
    pub fn freeze_frame(
        &self,
        clip_id: &str,
        at_ms: u64,
        duration_ms: u64,
        png: &Path,
    ) -> Result<(Snapshot, String)> {
        if !(100..=60_000).contains(&duration_ms) {
            return Err("Freeze duration must be 100..=60000 ms".into());
        }
        let snapshot = self.snapshot()?;
        let p = &snapshot.project;
        let main = p.main_video_track().ok_or("No main video track")?;
        let index = main
            .clips
            .iter()
            .position(|c| c.id == clip_id)
            .ok_or("Freeze frame works on main-track clips")?;
        let clip = &main.clips[index];
        if at_ms < clip.position_ms || at_ms > clip.position_ms + clip.duration() {
            return Err("The freeze point must be inside the clip".into());
        }
        let local = at_ms - clip.position_ms;
        let asset = p
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .ok_or("Unknown asset")?;
        let source = clip
            .source_at(local)
            .min(asset.duration_ms.saturating_sub(1));
        blyacut_engine::studio::extract_frame(asset, source, png)?;
        let still = blyacut_engine::probe(png)?;
        let still_id = still.id.clone();
        let (track_id, effects) = (main.id.clone(), clip.effects.clone());
        let imported = self.import_at(still, snapshot.revision)?;
        let freeze = id();
        let mut edits = vec![];
        // Split inside the clip; at an edge the still goes before or after it.
        let position = if local == 0 {
            index
        } else if local >= clip.duration() {
            index + 1
        } else {
            edits.push(Edit::SplitClip {
                clip_id: clip_id.into(),
                position_ms: at_ms,
                new_clip_id: id(),
            });
            index + 1
        };
        edits.extend([
            Edit::Append {
                asset_id: still_id,
                clip_id: freeze.clone(),
                track_id,
            },
            Edit::TrimClip {
                clip_id: freeze.clone(),
                start_ms: 0,
                end_ms: duration_ms,
            },
            Edit::ReorderClip {
                clip_id: freeze.clone(),
                index: position,
            },
            Edit::SetEffects {
                clip_id: freeze.clone(),
                effects,
            },
        ]);
        Ok((self.apply(&edits, imported.revision)?, freeze))
    }
    /// Render timed captions as text images in `dir` and lay them on a new overlay track
    /// named "Subtitles": one import step plus one edit step. Returns the track ID.
    pub fn add_captions(
        &self,
        mut captions: Vec<blyacut_core::captions::Caption>,
        position: &str,
        text_rgb: [u8; 3],
        backing: blyacut_engine::studio::Backing,
        dir: &Path,
    ) -> Result<(Snapshot, String)> {
        blyacut_core::captions::validate(&mut captions)?;
        let snapshot = self.snapshot()?;
        let center_y = blyacut_engine::studio::text_position(&snapshot.project, position)?;
        let batch = id();
        let mut assets = vec![];
        for (i, caption) in captions.iter().enumerate() {
            let png = dir.join(format!("caption-{batch}-{i:03}.png"));
            blyacut_engine::studio::text_overlay(
                &snapshot.project,
                caption.text.trim(),
                text_rgb,
                center_y,
                backing,
                &png,
            )?;
            assets.push(blyacut_engine::probe(&png)?);
        }
        let ids: Vec<String> = assets.iter().map(|a| a.id.clone()).collect();
        let imported = {
            let mut e = self.editor()?;
            if e.revision != snapshot.revision {
                return Err(stale(snapshot.revision, e.revision));
            }
            e.import_many(assets)?;
            Self::snapshot_of(&e)
        };
        let imported = self.changed(imported);
        let track_id = id();
        let mut edits = vec![Edit::AddTrack {
            track_id: track_id.clone(),
            name: "Subtitles".into(),
            kind: blyacut_core::TrackKind::Video,
        }];
        for (caption, asset_id) in captions.iter().zip(ids) {
            let clip_id = id();
            edits.extend([
                Edit::Append {
                    asset_id,
                    clip_id: clip_id.clone(),
                    track_id: track_id.clone(),
                },
                Edit::TrimClip {
                    clip_id: clip_id.clone(),
                    start_ms: 0,
                    end_ms: caption.end_ms - caption.start_ms,
                },
                Edit::MoveClip {
                    clip_id,
                    track_id: track_id.clone(),
                    position_ms: caption.start_ms,
                },
            ]);
        }
        Ok((self.apply(&edits, imported.revision)?, track_id))
    }
    pub fn render_job(&self) -> Result<Option<RenderJob>> {
        Ok(self
            .job
            .lock()
            .map_err(|e| e.to_string())?
            .as_ref()
            .map(|(j, _)| j.clone()))
    }
    pub fn export(self: &Arc<Self>, output: &Path, revision: Option<u64>) -> Result<RenderJob> {
        let snapshot = self.snapshot()?;
        if let Some(r) = revision.filter(|r| *r != snapshot.revision) {
            return Err(stale(r, snapshot.revision));
        }
        snapshot.project.validate()?;
        let cancel = Arc::new(AtomicBool::new(false));
        let job = RenderJob {
            id: id(),
            revision: snapshot.revision,
            progress: 0.0,
            status: "rendering".into(),
            error: None,
            output: Some(output.to_path_buf()),
            loudness: None,
            warnings: snapshot.project.warnings(),
        };
        {
            let mut slot = self.job.lock().map_err(|e| e.to_string())?;
            if slot.as_ref().is_some_and(|(j, _)| j.status == "rendering") {
                return Err("An export is already running".into());
            }
            *slot = Some((job.clone(), cancel.clone()));
        }
        (self.emit)(Event::Render(job.clone()));
        let session = self.clone();
        let path = output.to_path_buf();
        std::thread::spawn(move || {
            let result = blyacut_engine::render(&snapshot.project, &path, cancel, |progress| {
                session.update_job(progress, "rendering", None)
            });
            if result.is_ok() {
                session.measure(&path);
            }
            let status = if result.is_ok() { "complete" } else { "failed" };
            session.update_job(if result.is_ok() { 1.0 } else { 0.0 }, status, result.err());
        });
        Ok(job)
    }
    fn measure(&self, path: &Path) {
        let Ok(level) = blyacut_engine::studio::loudness(path) else {
            return;
        };
        if let Ok(mut slot) = self.job.lock() {
            if let Some((job, _)) = slot.as_mut() {
                if level.peak_db > -0.5 {
                    job.warnings.push(format!(
                        "Audio peaks at {:.1} dBFS and hits the limiter; lower gain_milli on loud clips",
                        level.peak_db
                    ));
                }
                if level.mean_db < -35.0 {
                    job.warnings.push(format!(
                        "Audio is very quiet (mean {:.1} dBFS); raise gain or add normalize",
                        level.mean_db
                    ));
                }
                job.loudness = Some(level);
            }
        }
    }
    /// Blocks until no export is rendering (headless shutdown waits for the file).
    pub fn wait_for_export(&self) {
        while self
            .render_job()
            .ok()
            .flatten()
            .is_some_and(|j| j.status == "rendering")
        {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    fn update_job(&self, progress: f64, status: &str, error: Option<String>) {
        let job = if let Ok(mut slot) = self.job.lock() {
            if let Some((job, _)) = slot.as_mut() {
                job.progress = progress;
                job.status = status.into();
                job.error = error;
                Some(job.clone())
            } else {
                None
            }
        } else {
            None
        };
        if let Some(job) = job {
            (self.emit)(Event::Render(job));
        }
    }
    pub fn cancel(&self) -> Result<()> {
        if let Some((_, flag)) = self.job.lock().map_err(|e| e.to_string())?.as_ref() {
            flag.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}
