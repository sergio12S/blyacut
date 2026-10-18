//! Framework-independent, transactional editing model. All times are integer milliseconds.
pub mod effects;
use effects::EffectRecipe;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};
use uuid::Uuid;
pub type Result<T> = std::result::Result<T, String>;
pub fn id() -> String {
    Uuid::new_v4().to_string()
}
const MAX_TIME: u64 = 604_800_000; // One week, also safely representable in JS.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub has_audio: bool,
}
fn unity() -> u16 {
    1000
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Clip {
    pub id: String,
    pub asset_id: String,
    pub position_ms: u64,
    pub source_start_ms: u64,
    pub source_end_ms: u64,
    #[serde(default = "unity")]
    pub gain_milli: u16,
    #[serde(default)]
    pub fade_in_ms: u64,
    #[serde(default)]
    pub fade_out_ms: u64,
    #[serde(default)]
    pub effects: Vec<EffectRecipe>,
}
impl Clip {
    pub fn duration(&self) -> u64 {
        self.source_end_ms.saturating_sub(self.source_start_ms)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Video,
    Audio,
    Text,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub kind: TrackKind,
    pub clips: Vec<Clip>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub settings: Settings,
    pub assets: Vec<Asset>,
    pub tracks: Vec<Track>,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            version: 3,
            id: id(),
            name: "Untitled project".into(),
            settings: Settings {
                width: 1920,
                height: 1080,
                fps_num: 30,
                fps_den: 1,
            },
            assets: vec![],
            tracks: vec![
                Track {
                    id: id(),
                    name: "Video 1".into(),
                    kind: TrackKind::Video,
                    clips: vec![],
                },
                Track {
                    id: id(),
                    name: "Audio 1".into(),
                    kind: TrackKind::Audio,
                    clips: vec![],
                },
            ],
        }
    }
}
impl Project {
    pub fn duration(&self) -> u64 {
        self.tracks
            .iter()
            .flat_map(|t| &t.clips)
            .map(|c| c.position_ms + c.duration())
            .max()
            .unwrap_or(0)
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != 3 {
            return Err(format!("Unsupported project version {}", self.version));
        }
        if self.assets.len() > 4096
            || self.tracks.len() > 32
            || self.tracks.iter().map(|t| t.clips.len()).sum::<usize>() > 4096
        {
            return Err("Project exceeds asset, track or clip limits".into());
        }
        let s = &self.settings;
        if s.width < 2
            || s.height < 2
            || s.width > 7680
            || s.height > 4320
            || !s.width.is_multiple_of(2)
            || !s.height.is_multiple_of(2)
            || s.fps_num == 0
            || s.fps_den == 0
            || s.fps_num > 240 * s.fps_den.min(1000)
            || s.fps_den > 1000
        {
            return Err("Invalid output settings".into());
        }
        let mut ids = HashSet::new();
        if self.id.is_empty() || !ids.insert(self.id.as_str()) {
            return Err("Invalid project ID".into());
        }
        for a in &self.assets {
            if a.id.is_empty()
                || !ids.insert(&a.id)
                || a.duration_ms == 0
                || a.duration_ms > MAX_TIME
                || (a.width == 0) != (a.height == 0)
                || (a.width == 0 && !a.has_audio)
            {
                return Err("Invalid or duplicate asset".into());
            }
        }
        if self.tracks.is_empty() {
            return Err("Project needs a track".into());
        }
        for t in &self.tracks {
            if t.id.is_empty() || !ids.insert(&t.id) {
                return Err("Duplicate track ID".into());
            }
            let mut end = 0;
            for c in &t.clips {
                let a = self
                    .assets
                    .iter()
                    .find(|a| a.id == c.asset_id)
                    .ok_or("Clip references unknown asset")?;
                if c.id.is_empty()
                    || !ids.insert(&c.id)
                    || c.source_start_ms >= c.source_end_ms
                    || c.source_end_ms > a.duration_ms
                    || c.position_ms > MAX_TIME
                    || c.duration() > MAX_TIME - c.position_ms
                {
                    return Err("Invalid clip range or ID".into());
                }
                if c.effects.len() > 8
                    || c.effects.iter().map(|r| r.steps.len()).sum::<usize>() > 32
                {
                    return Err("Clip effect chain exceeds limits".into());
                }
                for recipe in &c.effects {
                    recipe.validate()?;
                    if t.kind == TrackKind::Audio && recipe.steps.iter().any(|s| s.is_video()) {
                        return Err("Video effect cannot be applied to an audio-only track".into());
                    }
                }
                if c.gain_milli > 1000
                    || c.fade_in_ms > c.duration()
                    || c.fade_out_ms > c.duration() - c.fade_in_ms
                {
                    return Err("Invalid clip gain or fades".into());
                }
                if (t.kind == TrackKind::Video && a.width == 0)
                    || (t.kind == TrackKind::Audio && !a.has_audio)
                    || t.kind == TrackKind::Text
                {
                    return Err("Media is incompatible with track type".into());
                }
                if c.position_ms < end {
                    return Err("Clips overlap or are out of order on a track".into());
                }
                end = c.position_ms + c.duration();
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    SetEffects {
        clip_id: String,
        effects: Vec<EffectRecipe>,
    },
    AddTrack {
        track_id: String,
        name: String,
        kind: TrackKind,
    },
    MoveClip {
        clip_id: String,
        track_id: String,
        position_ms: u64,
    },
    SetAudio {
        clip_id: String,
        gain_milli: u16,
        fade_in_ms: u64,
        fade_out_ms: u64,
    },
    Append {
        asset_id: String,
        clip_id: String,
        track_id: String,
    },
    SplitClip {
        clip_id: String,
        position_ms: u64,
        new_clip_id: String,
    },
    TrimClip {
        clip_id: String,
        start_ms: u64,
        end_ms: u64,
    },
    DeleteClip {
        clip_id: String,
    },
    ReorderClip {
        clip_id: String,
        index: usize,
    },
}
fn ripple(t: &mut Track) {
    let mut pos = 0;
    for c in &mut t.clips {
        c.position_ms = pos;
        pos += c.duration();
    }
}
fn apply(p: &mut Project, edit: Edit) -> Result<()> {
    match &edit {
        Edit::AddTrack {
            track_id,
            name,
            kind,
        } => {
            if *kind == TrackKind::Text
                || name.is_empty()
                || name.len() > 128
                || p.tracks.len() >= 32
            {
                return Err("Invalid track or track limit reached".into());
            }
            p.tracks.push(Track {
                id: track_id.clone(),
                name: name.clone(),
                kind: kind.clone(),
                clips: vec![],
            });
            return p.validate();
        }
        Edit::MoveClip {
            clip_id,
            track_id,
            position_ms,
        } => {
            let source = p
                .tracks
                .iter()
                .position(|t| t.clips.iter().any(|c| &c.id == clip_id))
                .ok_or("Unknown clip")?;
            let target = p
                .tracks
                .iter()
                .position(|t| &t.id == track_id)
                .ok_or("Unknown track")?;
            let index = p.tracks[source]
                .clips
                .iter()
                .position(|c| &c.id == clip_id)
                .ok_or("Unknown clip")?;
            let mut clip = p.tracks[source].clips.remove(index);
            clip.position_ms = *position_ms;
            p.tracks[target].clips.push(clip);
            p.tracks[target].clips.sort_by_key(|c| c.position_ms);
            return p.validate();
        }
        _ => {}
    }
    if let Edit::Append {
        asset_id,
        clip_id,
        track_id,
    } = edit
    {
        let a = p
            .assets
            .iter()
            .find(|a| a.id == asset_id)
            .ok_or("Unknown asset")?;
        let t = p
            .tracks
            .iter_mut()
            .find(|t| t.id == track_id)
            .ok_or("Unknown track")?;
        let pos = t
            .clips
            .last()
            .map(|c| c.position_ms + c.duration())
            .unwrap_or(0);
        t.clips.push(Clip {
            id: clip_id,
            asset_id,
            position_ms: pos,
            source_start_ms: 0,
            source_end_ms: a.duration_ms,
            gain_milli: 1000,
            fade_in_ms: 0,
            fade_out_ms: 0,
            effects: vec![],
        });
    } else {
        let cid = match &edit {
            Edit::SplitClip { clip_id, .. }
            | Edit::TrimClip { clip_id, .. }
            | Edit::DeleteClip { clip_id }
            | Edit::ReorderClip { clip_id, .. }
            | Edit::SetAudio { clip_id, .. }
            | Edit::SetEffects { clip_id, .. } => clip_id,
            _ => unreachable!(),
        };
        let t = p
            .tracks
            .iter_mut()
            .find(|t| t.clips.iter().any(|c| &c.id == cid))
            .ok_or("Unknown clip")?;
        let i = t
            .clips
            .iter()
            .position(|c| &c.id == cid)
            .ok_or("Unknown clip")?;
        match edit {
            Edit::SplitClip {
                position_ms,
                new_clip_id,
                ..
            } => {
                let c = &mut t.clips[i];
                if position_ms <= c.position_ms || position_ms >= c.position_ms + c.duration() {
                    return Err("Split must be inside the clip (timeline milliseconds)".into());
                }
                let offset = position_ms - c.position_ms;
                if offset < c.fade_in_ms || c.duration() - offset < c.fade_out_ms {
                    return Err("Cannot split inside a fade; adjust or clear the fade first".into());
                }
                let cut = c.source_start_ms + offset;
                let right = Clip {
                    id: new_clip_id,
                    asset_id: c.asset_id.clone(),
                    position_ms,
                    source_start_ms: cut,
                    source_end_ms: c.source_end_ms,
                    gain_milli: c.gain_milli,
                    fade_in_ms: 0,
                    fade_out_ms: c.fade_out_ms.min(c.source_end_ms - cut),
                    effects: c.effects.clone(),
                };
                c.source_end_ms = cut;
                c.fade_in_ms = c.fade_in_ms.min(c.duration());
                c.fade_out_ms = 0;
                t.clips.insert(i + 1, right);
            }
            Edit::TrimClip {
                start_ms, end_ms, ..
            } => {
                let c = &mut t.clips[i];
                if start_ms >= end_ms {
                    return Err("Trim needs a positive source range".into());
                }
                c.source_start_ms = start_ms;
                c.source_end_ms = end_ms;
                if t.kind == TrackKind::Video {
                    ripple(t);
                }
            }
            Edit::DeleteClip { .. } => {
                t.clips.remove(i);
                if t.kind == TrackKind::Video {
                    ripple(t);
                }
            }
            Edit::SetEffects { effects, .. } => {
                t.clips[i].effects = effects;
            }
            Edit::SetAudio {
                gain_milli,
                fade_in_ms,
                fade_out_ms,
                ..
            } => {
                let c = &mut t.clips[i];
                c.gain_milli = gain_milli;
                c.fade_in_ms = fade_in_ms;
                c.fade_out_ms = fade_out_ms;
            }
            Edit::ReorderClip { index, .. } => {
                if t.kind != TrackKind::Video {
                    return Err("Use move_clip to position audio".into());
                }
                if index >= t.clips.len() {
                    return Err("Invalid clip index".into());
                }
                let c = t.clips.remove(i);
                t.clips.insert(index, c);
                if t.kind == TrackKind::Video {
                    ripple(t);
                }
            }
            _ => unreachable!(),
        }
    }
    p.validate()
}
#[derive(Default)]
pub struct Editor {
    pub project: Project,
    undo: Vec<Project>,
    redo: Vec<Project>,
    pub revision: u64,
}
impl Editor {
    pub fn new(project: Project) -> Result<Self> {
        project.validate()?;
        Ok(Self {
            project,
            ..Self::default()
        })
    }
    fn commit(&mut self, p: Project) {
        self.undo.push(std::mem::replace(&mut self.project, p));
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.revision += 1;
    }
    pub fn preview(&self, edits: &[Edit]) -> Result<Project> {
        if edits.is_empty() || edits.len() > 100 {
            return Err("Batch must contain 1–100 edits".into());
        }
        let mut next = self.project.clone();
        for edit in edits {
            apply(&mut next, edit.clone())?;
        }
        Ok(next)
    }
    pub fn execute(&mut self, edit: Edit) -> Result<()> {
        self.batch(&[edit], self.revision)
    }
    pub fn batch(&mut self, edits: &[Edit], expected_revision: u64) -> Result<()> {
        if expected_revision != self.revision {
            return Err(format!(
                "Stale revision: expected {}, current {}",
                expected_revision, self.revision
            ));
        }
        let next = self.preview(edits)?;
        self.commit(next);
        Ok(())
    }
    pub fn replace(&mut self, project: Project) -> Result<()> {
        project.validate()?;
        self.project = project;
        self.undo.clear();
        self.redo.clear();
        self.revision += 1;
        Ok(())
    }
    pub fn import(&mut self, asset: Asset) -> Result<()> {
        let mut next = self.project.clone();
        next.assets.push(asset);
        next.validate()?;
        self.commit(next);
        Ok(())
    }
    pub fn undo(&mut self) {
        if let Some(p) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.project, p));
            self.revision += 1;
        }
    }
    pub fn redo(&mut self) {
        if let Some(p) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.project, p));
            self.revision += 1;
        }
    }
}

pub fn save(project: &Project, path: &std::path::Path) -> Result<()> {
    project.validate()?;
    if path.extension().and_then(|s| s.to_str()) != Some("blyacut") {
        return Err("Project filename must end in .blyacut".into());
    }
    let parent = path.parent().ok_or("Project needs a parent directory")?;
    let mut portable = project.clone();
    for a in &mut portable.assets {
        if let Ok(relative) = a.path.strip_prefix(parent) {
            a.path = relative.into();
        }
    }
    let temp = parent.join(format!(".blyacut-{}.tmp", id()));
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(&portable).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(temp);
        return Err(e.to_string());
    }
    Ok(())
}
pub fn load(path: &std::path::Path) -> Result<Project> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 16 * 1024 * 1024 {
        return Err("Project exceeds 16 MiB".into());
    }
    let mut p: Project = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if p.version == 1 || p.version == 2 {
        p.version = 3;
    }
    p.validate()?;
    for a in &mut p.assets {
        if a.path.is_relative() {
            a.path = path.parent().ok_or("Missing parent")?.join(&a.path);
        }
    }
    Ok(p)
}
