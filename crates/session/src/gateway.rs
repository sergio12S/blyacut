//! The agent adapter limits filesystem access. In review mode removals/large batches need a human;
//! an autonomous gateway (explicitly granted by whoever starts it) applies them directly.
use crate::Session;
use blyacut_core::{Edit, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Batch {
    expected_revision: u64,
    edits: Vec<Edit>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Import {
    path: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateRecipe {
    recipe: blyacut_core::effects::EffectRecipe,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    output_path: PathBuf,
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preview {
    time_ms: u64,
    output_path: PathBuf,
    #[serde(default = "preview_width")]
    width: u32,
}
fn preview_width() -> u32 {
    960
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Title {
    text: String,
    output_path: PathBuf,
    duration_ms: u64,
    #[serde(default = "white")]
    text_color: String,
    #[serde(default = "black")]
    background_color: String,
    #[serde(default = "yes")]
    import: bool,
}
fn white() -> String {
    "#FFFFFF".into()
}
fn black() -> String {
    "#000000".into()
}
fn yes() -> bool {
    true
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextOverlay {
    text: String,
    output_path: PathBuf,
    #[serde(default = "bottom")]
    position: String,
    #[serde(default = "white")]
    text_color: String,
    #[serde(default = "plate")]
    style: String,
    #[serde(default = "yes")]
    import: bool,
    place: Option<Place>,
}
/// Put a freshly imported overlay on a track in one step.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Place {
    track_id: String,
    position_ms: u64,
    duration_ms: u64,
}
fn plate() -> String {
    "box".into()
}
fn bottom() -> String {
    "bottom".into()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtractAudio {
    clip_id: String,
    output_path: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BassVisualizer {
    input_path: PathBuf,
    output_path: PathBuf,
    #[serde(default = "visualizer_intensity")]
    intensity: u32,
}
fn caption_lines() -> String {
    "lines".into()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DirtyPhonk {
    input_path: PathBuf,
    output_path: PathBuf,
    #[serde(default = "visualizer_intensity")]
    intensity: u32,
    #[serde(default)]
    options: blyacut_engine::visualizer::PhonkOptions,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleasePackage {
    input_path: PathBuf,
    output_dir: PathBuf,
    prefix: String,
    title: String,
    #[serde(default = "short_seconds")]
    short_seconds: u32,
    starts_ms: Option<Vec<u64>>,
    captions_srt: Option<String>,
    look: Option<String>,
    #[serde(default = "visualizer_intensity")]
    intensity: u32,
    #[serde(default = "caption_lines")]
    caption_mode: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseBatch {
    jobs: Vec<ReleasePackage>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MusicJobId {
    job_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartLoopMix {
    segments: Vec<blyacut_engine::loopmix::Segment>,
    output_path: PathBuf,
    #[serde(default = "loop_duration")]
    duration_seconds: u32,
    #[serde(default = "loop_fade")]
    fade_ms: u32,
    #[serde(default = "loop_width")]
    width: u32,
    #[serde(default = "loop_height")]
    height: u32,
}
fn loop_duration() -> u32 {
    600
}
fn loop_fade() -> u32 {
    1000
}
fn loop_width() -> u32 {
    1280
}
fn loop_height() -> u32 {
    720
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MusicAnalysis {
    input_path: PathBuf,
    #[serde(default = "short_seconds")]
    seconds: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MusicFolder {
    folder: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MusicPreviews {
    input_path: PathBuf,
    output_dir: PathBuf,
    prefix: String,
    start_ms: u64,
    #[serde(default = "preview_seconds")]
    seconds: u32,
    #[serde(default = "visualizer_intensity")]
    intensity: u32,
}
fn preview_seconds() -> u32 {
    10
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MusicThumbnails {
    input_path: PathBuf,
    output_dir: PathBuf,
    prefix: String,
    title: String,
    times_ms: Vec<u64>,
}
fn short_seconds() -> u32 {
    15
}
fn visualizer_intensity() -> u32 {
    55
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Captions {
    #[serde(default)]
    captions: Vec<blyacut_core::captions::Caption>,
    srt: Option<String>,
    output_dir: PathBuf,
    #[serde(default = "bottom")]
    position: String,
    #[serde(default = "white")]
    text_color: String,
    #[serde(default = "plate")]
    style: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Freeze {
    clip_id: String,
    at_ms: u64,
    duration_ms: u64,
    output_path: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectFile {
    path: PathBuf,
}
#[derive(Clone, Serialize)]
pub struct GatewayInfo {
    pub allowed_root: PathBuf,
    /// "review": removals and >20-edit batches need desktop approval. "autonomous": applied directly.
    pub mode: &'static str,
}
pub struct Gateway {
    pub session: Arc<Session>,
    root: PathBuf,
    autonomous: bool,
    music_queue: Arc<crate::music_jobs::MusicQueue>,
}
impl Gateway {
    pub fn new(session: Arc<Session>, root: &Path) -> Result<Self> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        if !root.is_dir() {
            return Err("Choose a directory for agent access".into());
        }
        Ok(Self {
            session,
            root,
            autonomous: false,
            music_queue: Arc::new(crate::music_jobs::MusicQueue::default()),
        })
    }
    /// Grant the agent full editing authority, with no human review step.
    pub fn autonomous(mut self, enabled: bool) -> Self {
        self.autonomous = enabled;
        self
    }
    pub fn info(&self) -> GatewayInfo {
        GatewayInfo {
            allowed_root: self.root.clone(),
            mode: if self.autonomous {
                "autonomous"
            } else {
                "review"
            },
        }
    }
    pub fn wait_for_music_jobs(&self) {
        self.music_queue.wait();
    }

    fn music_request(&self, a: ReleasePackage) -> Result<crate::music_jobs::Request> {
        let input = self.input(&a.input_path)?;
        let output_dir = self.input(&a.output_dir)?;
        if !output_dir.is_dir() {
            return Err("output_dir must be an existing directory".into());
        }
        if a.prefix.is_empty()
            || a.prefix.len() > 48
            || !a
                .prefix
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err("Invalid prefix".into());
        }
        if a.title.trim().is_empty()
            || a.title.chars().count() > 100
            || a.title.lines().count() > 2
            || !(5..=30).contains(&a.short_seconds)
        {
            return Err("Invalid title or short_seconds".into());
        }
        for suffix in ["full", "short-1", "short-2", "short-3"] {
            if output_dir
                .join(format!("{}-{suffix}.mp4", a.prefix))
                .exists()
            {
                return Err("Output file already exists".into());
            }
        }
        if a.look
            .as_ref()
            .is_some_and(|look| !["dirty", "neon", "vhs", "warehouse"].contains(&look.as_str()))
            || !(1..=100).contains(&a.intensity)
        {
            return Err("Invalid look or intensity".into());
        }
        if a.look.is_some() && output_dir.join(format!("{}-styled.mp4", a.prefix)).exists() {
            return Err("Styled output file already exists".into());
        }
        if !["lines", "word", "none"].contains(&a.caption_mode.as_str()) {
            return Err("caption_mode must be lines, word or none".into());
        }
        if a.caption_mode == "none" && a.captions_srt.is_some() {
            return Err("caption_mode none cannot be combined with captions_srt".into());
        }
        if a.caption_mode == "word" && a.captions_srt.is_none() {
            return Err("Word mode requires supplied word-timed SRT".into());
        }
        if let Some(srt) = &a.captions_srt {
            let mut captions = blyacut_core::captions::parse_srt(srt)?;
            blyacut_core::captions::validate(&mut captions)?;
            if captions.iter().any(|c| c.text.lines().count() > 2) {
                return Err("Supplied captions must have at most two lines per cue".into());
            }
            if a.caption_mode == "word"
                && captions
                    .iter()
                    .any(|c| c.text.split_whitespace().count() != 1)
            {
                return Err("Word mode needs exactly one timed word per SRT cue".into());
            }
        }
        Ok(crate::music_jobs::Request {
            input,
            output_dir,
            prefix: a.prefix,
            title: a.title,
            seconds: a.short_seconds,
            starts_ms: a.starts_ms,
            srt: a.captions_srt,
            look: a.look,
            intensity: a.intensity,
            caption_mode: a.caption_mode,
        })
    }
    pub fn input(&self, path: &Path) -> Result<PathBuf> {
        if !path.is_absolute() {
            return Err("Agent paths must be absolute".into());
        }
        let canonical = path.canonicalize().map_err(|e| e.to_string())?;
        if !canonical.starts_with(&self.root) {
            return Err("Path outside authorized folder".into());
        }
        Ok(canonical)
    }
    pub fn output(&self, path: &Path) -> Result<PathBuf> {
        self.new_file(path, "mp4")
    }
    fn new_file(&self, path: &Path, ext: &str) -> Result<PathBuf> {
        if !path.is_absolute() || path.exists() {
            return Err("Output needs an absolute, new filename".into());
        }
        self.inside(path, ext)
    }
    /// Absolute path with the given extension whose parent lies in the authorized folder.
    fn inside(&self, path: &Path, ext: &str) -> Result<PathBuf> {
        if !path.is_absolute() {
            return Err("Agent paths must be absolute".into());
        }
        let parent = self.input(path.parent().ok_or("Missing output directory")?)?;
        let name = path.file_name().ok_or("Missing output filename")?;
        if path.extension().and_then(|s| s.to_str()) != Some(ext) {
            return Err(format!("Output must end in .{ext}"));
        }
        Ok(parent.join(name))
    }
    fn with_warnings(snapshot: crate::Snapshot) -> Value {
        let warnings = snapshot.project.warnings();
        let mut value = json!(snapshot);
        value["warnings"] = json!(warnings);
        value
    }
    pub fn call(&self, name: &str, args: Value) -> Result<Value> {
        match name {
            "list_effect_recipes" => {
                empty(args)?;
                let builtin = blyacut_core::effects::builtin_recipes();
                let directory = self.root.join("effects");
                if !directory.exists() {
                    return Ok(json!({"recipes":[],"warnings":[],"builtin":builtin}));
                }
                let directory = self.input(&directory)?;
                let mut catalog = json!(crate::effects::load(&directory)?);
                catalog["builtin"] = json!(builtin);
                Ok(catalog)
            }
            "create_effect_recipe" => {
                let request: CreateRecipe = parse(args)?;
                request.recipe.validate()?;
                let directory = self.root.join("effects");
                if !directory.exists() {
                    std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
                }
                let directory = self.input(&directory)?;
                crate::effects::create(&directory, &request.recipe)?;
                Ok(
                    json!({"recipe":request.recipe,"path":directory.join(format!("{}.json",request.recipe.id))}),
                )
            }
            "get_project" => {
                empty(args)?;
                Ok(json!(self.session.snapshot()?))
            }
            "get_timeline" => {
                empty(args)?;
                let s = self.session.snapshot()?;
                Ok(
                    json!({"revision":s.revision,"tracks":s.project.tracks,"duration_ms":s.project.duration(),"warnings":s.project.warnings()}),
                )
            }
            "get_session" => {
                empty(args)?;
                Ok(
                    json!({"access":self.info(),"render_job":self.session.render_job()?,"pending_proposal":self.session.pending()?}),
                )
            }
            "import_media" => {
                let a: Import = parse(args)?;
                let path = self.input(&a.path)?;
                let revision = self.session.snapshot()?.revision;
                let asset = blyacut_engine::probe(&path)?;
                Ok(Self::with_warnings(
                    self.session.import_at(asset, revision)?,
                ))
            }
            "apply_edits" => {
                let b: Batch = parse(args)?;
                if !self.autonomous
                    && (b.edits.len() > 20
                        || b.edits.iter().any(|e| matches!(e, Edit::DeleteClip { .. })))
                {
                    return Err(
                        "Removal or batch >20 edits requires propose_edits and desktop approval"
                            .into(),
                    );
                }
                if b.edits.len() > 100 {
                    return Err("At most 100 edits per batch".into());
                }
                Ok(Self::with_warnings(
                    self.session.apply(&b.edits, b.expected_revision)?,
                ))
            }
            "propose_edits" => {
                if self.autonomous {
                    return Err(
                        "Autonomous session has no human reviewer: use apply_edits directly".into(),
                    );
                }
                let b: Batch = parse(args)?;
                Ok(json!(self.session.propose(b.edits, b.expected_revision)?))
            }
            "undo" | "redo" => {
                let r: Revision = parse(args)?;
                Ok(json!(self
                    .session
                    .history(name == "redo", Some(r.expected_revision))?))
            }
            "export_video" => {
                let a: Export = parse(args)?;
                let path = self.output(&a.output_path)?;
                Ok(json!(self
                    .session
                    .export(&path, Some(a.expected_revision))?))
            }
            "preview_frame" => {
                let a: Preview = parse(args)?;
                let path = self.new_file(&a.output_path, "png")?;
                let s = self.session.snapshot()?;
                blyacut_engine::studio::preview_frame(&s.project, a.time_ms, &path, a.width)?;
                Ok(json!({"path":path,"time_ms":a.time_ms,"revision":s.revision}))
            }
            "create_title_card" => {
                let a: Title = parse(args)?;
                let path = self.new_file(&a.output_path, "mp4")?;
                let style = blyacut_engine::studio::TitleStyle {
                    text_rgb: blyacut_engine::studio::parse_hex(&a.text_color)?,
                    background_rgb: blyacut_engine::studio::parse_hex(&a.background_color)?,
                };
                let s = self.session.snapshot()?;
                blyacut_engine::studio::title_card(
                    &s.project,
                    &a.text,
                    a.duration_ms,
                    &style,
                    &path,
                )?;
                if !a.import {
                    return Ok(json!({"path":path}));
                }
                let asset = blyacut_engine::probe(&path)?;
                let asset_id = asset.id.clone();
                let mut value = Self::with_warnings(self.session.import_at(asset, s.revision)?);
                value["path"] = json!(path);
                value["asset_id"] = json!(asset_id);
                Ok(value)
            }
            "create_text_overlay" => {
                let a: TextOverlay = parse(args)?;
                let path = self.new_file(&a.output_path, "png")?;
                let s = self.session.snapshot()?;
                let center_y = blyacut_engine::studio::text_position(&s.project, &a.position)?;
                blyacut_engine::studio::text_overlay(
                    &s.project,
                    &a.text,
                    blyacut_engine::studio::parse_hex(&a.text_color)?,
                    center_y,
                    a.style.parse()?,
                    &path,
                )?;
                if !a.import {
                    return Ok(json!({"path":path}));
                }
                let asset = blyacut_engine::probe(&path)?;
                let asset_id = asset.id.clone();
                let mut snapshot = self.session.import_at(asset, s.revision)?;
                let mut clip_id = None;
                if let Some(place) = a.place {
                    let id = blyacut_core::id();
                    snapshot = self.session.apply(
                        &[
                            Edit::Append {
                                asset_id: asset_id.clone(),
                                clip_id: id.clone(),
                                track_id: place.track_id.clone(),
                            },
                            Edit::TrimClip {
                                clip_id: id.clone(),
                                start_ms: 0,
                                end_ms: place.duration_ms,
                            },
                            Edit::MoveClip {
                                clip_id: id.clone(),
                                track_id: place.track_id,
                                position_ms: place.position_ms,
                            },
                        ],
                        snapshot.revision,
                    )?;
                    clip_id = Some(id);
                }
                let mut value = Self::with_warnings(snapshot);
                value["path"] = json!(path);
                value["asset_id"] = json!(asset_id);
                value["clip_id"] = json!(clip_id);
                Ok(value)
            }
            "render_release_package" => {
                let a: ReleasePackage = parse(args)?;
                let r = self.music_request(a)?;
                crate::music_jobs::execute(&r, &std::sync::atomic::AtomicBool::new(false), |_| {})
            }
            "start_release_jobs" => {
                let batch: ReleaseBatch = parse(args)?;
                if batch.jobs.is_empty() || batch.jobs.len() > 10 {
                    return Err("Submit 1..=10 release jobs".into());
                }
                let jobs = batch
                    .jobs
                    .into_iter()
                    .map(|a| self.music_request(a))
                    .collect::<Result<Vec<_>>>()?;
                Ok(
                    json!({"jobs":self.music_queue.submit(jobs)?,"note":"Sequential background queue; completed_steps counts 4 or 5 output files. Partial files may remain after failure or cancellation."}),
                )
            }
            "start_loop_mix" => {
                let a: StartLoopMix = parse(args)?;
                let output = self.output(&a.output_path)?;
                let segments = a
                    .segments
                    .into_iter()
                    .map(|segment| {
                        Ok(blyacut_engine::loopmix::Segment {
                            path: self.input(&segment.path)?,
                            start_ms: segment.start_ms,
                            end_ms: segment.end_ms,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let request = blyacut_engine::loopmix::Request {
                    segments,
                    output,
                    duration_seconds: a.duration_seconds,
                    fade_ms: a.fade_ms,
                    width: a.width,
                    height: a.height,
                };
                Ok(
                    json!({"job":self.music_queue.submit_loop(request)?,"note":"Queued; poll get_music_jobs. The two stages prepare a loop cycle and encode the final mix. Partial output may remain on cancellation."}),
                )
            }
            "analyze_music_candidates" => {
                let a: MusicAnalysis = parse(args)?;
                let input = self.input(&a.input_path)?;
                Ok(
                    json!({"candidates":blyacut_engine::music::candidates(&input,a.seconds)?,"note":"Bass-energy, rising-energy and onset-density heuristics only; listen before selecting a chorus or drop."}),
                )
            }
            "list_music_inputs" => {
                let a: MusicFolder = parse(args)?;
                let folder = self.input(&a.folder)?;
                if !folder.is_dir() {
                    return Err("folder must be a directory".into());
                }
                let mut paths = Vec::new();
                for entry in std::fs::read_dir(&folder).map_err(|e| e.to_string())? {
                    let entry = entry.map_err(|e| e.to_string())?;
                    let p = entry.path();
                    if !p.extension().and_then(|s| s.to_str()).is_some_and(|ext| {
                        ["mp4", "mov", "m4v", "mkv", "webm"]
                            .contains(&ext.to_ascii_lowercase().as_str())
                    }) {
                        continue;
                    }
                    if let Ok(path) = self.input(&p) {
                        if path.is_file() {
                            paths.push(path);
                        }
                    }
                    if paths.len() > 100 {
                        return Err(
                            "Folder has more than 100 video files; use a smaller folder".into()
                        );
                    }
                }
                paths.sort();
                paths.dedup();
                Ok(
                    json!({"paths":paths,"note":"Video files only, non-recursive; probe/import each file before rendering. Audio-only tracks require a video or cover first."}),
                )
            }
            "render_music_previews" => {
                let a: MusicPreviews = parse(args)?;
                let input = self.input(&a.input_path)?;
                let dir = self.input(&a.output_dir)?;
                Ok(
                    json!({"paths":blyacut_engine::music::previews(&input,&dir,&a.prefix,a.start_ms,a.seconds,a.intensity)?,"looks":["dirty","neon","vhs"],"note":"Short standalone style previews, source audio re-encoded for accurate seeking."}),
                )
            }
            "create_music_thumbnails" => {
                let a: MusicThumbnails = parse(args)?;
                let input = self.input(&a.input_path)?;
                let dir = self.input(&a.output_dir)?;
                let paths = blyacut_engine::music::thumbnails(
                    &input,
                    &dir,
                    &a.prefix,
                    &a.title,
                    &a.times_ms,
                )?;
                Ok(
                    json!({"paths":paths,"title_ideas":[a.title,format!("{} | Visualizer",a.title)],"description_draft":format!("{}\n\nСлушай полный трек и делись впечатлениями.",a.title),"note":"Simple frame-and-text variants and editable text templates, not generated artwork; review before publishing."}),
                )
            }
            "get_music_jobs" => {
                empty(args)?;
                Ok(json!({"jobs":self.music_queue.list()?}))
            }
            "cancel_music_job" => {
                let a: MusicJobId = parse(args)?;
                Ok(json!({"job":self.music_queue.cancel(&a.job_id)?}))
            }
            "render_dirty_phonk" => {
                let a: DirtyPhonk = parse(args)?;
                let input = self.input(&a.input_path)?;
                let output = self.output(&a.output_path)?;
                let result = blyacut_engine::visualizer::render_phonk(
                    &input,
                    &output,
                    a.intensity,
                    &a.options,
                )?;
                Ok(
                    json!({"path":output,"analysis":result,"intensity":a.intensity,"note":"Standalone MP4; original audio stream copied. Automatic drop is an estimate."}),
                )
            }
            "render_bass_visualizer" => {
                let a: BassVisualizer = parse(args)?;
                let input = self.input(&a.input_path)?;
                let output = self.output(&a.output_path)?;
                let windows = blyacut_engine::visualizer::render(&input, &output, a.intensity)?;
                Ok(
                    json!({"path":output,"analysis_windows":windows,"intensity":a.intensity,"note":"New standalone video; the session timeline was not changed. Audio was stream-copied."}),
                )
            }
            "extract_audio" => {
                let a: ExtractAudio = parse(args)?;
                let wav = self.new_file(&a.output_path, "wav")?;
                let s = self.session.snapshot()?;
                let clip = s
                    .project
                    .tracks
                    .iter()
                    .flat_map(|t| &t.clips)
                    .find(|c| c.id == a.clip_id)
                    .ok_or("Unknown clip")?;
                if clip.is_retimed() {
                    return Err("Extract audio from a normal-speed, forward clip".into());
                }
                let asset = s
                    .project
                    .assets
                    .iter()
                    .find(|x| x.id == clip.asset_id)
                    .ok_or("Unknown asset")?;
                blyacut_engine::studio::extract_audio(
                    asset,
                    clip.source_start_ms,
                    clip.span(),
                    &wav,
                )?;
                Ok(json!({
                    "path": wav,
                    "format": "WAV PCM 16-bit, 16 kHz, mono",
                    "timeline_offset_ms": clip.position_ms,
                    "duration_ms": clip.duration(),
                    "note": "WAV time 0 is timeline_offset_ms; add it to your transcript timings before add_captions"
                }))
            }
            "add_captions" => {
                let a: Captions = parse(args)?;
                let dir = self.input(&a.output_dir)?;
                if !dir.is_dir() {
                    return Err(
                        "output_dir must be an existing folder inside the authorized folder".into(),
                    );
                }
                let captions = match (a.srt, a.captions.is_empty()) {
                    (Some(srt), true) => blyacut_core::captions::parse_srt(&srt)?,
                    (None, false) => a.captions,
                    _ => return Err("Pass either captions or srt".into()),
                };
                let (snapshot, track_id) = self.session.add_captions(
                    captions,
                    &a.position,
                    blyacut_engine::studio::parse_hex(&a.text_color)?,
                    a.style.parse()?,
                    &dir,
                )?;
                let mut value = Self::with_warnings(snapshot);
                value["track_id"] = json!(track_id);
                Ok(value)
            }
            "freeze_frame" => {
                let a: Freeze = parse(args)?;
                let png = self.new_file(&a.output_path, "png")?;
                let (snapshot, clip_id) =
                    self.session
                        .freeze_frame(&a.clip_id, a.at_ms, a.duration_ms, &png)?;
                let mut value = Self::with_warnings(snapshot);
                value["clip_id"] = json!(clip_id);
                value["path"] = json!(png);
                Ok(value)
            }
            "save_project" => {
                let a: ProjectFile = parse(args)?;
                let path = self.inside(&a.path, "blyacut")?;
                let s = self.session.snapshot()?;
                blyacut_core::save(&s.project, &path)?;
                Ok(json!({"path":path,"revision":s.revision}))
            }
            "open_project" => {
                let a: ProjectFile = parse(args)?;
                let path = self.input(&a.path)?;
                let project = blyacut_core::load(&path)?;
                for asset in &project.assets {
                    self.input(&asset.path).map_err(|_| {
                        format!("Project media outside authorized folder: {}", asset.name)
                    })?;
                }
                Ok(Self::with_warnings(self.session.replace(project)?))
            }
            "cancel_export" => {
                empty(args)?;
                self.session.cancel()?;
                Ok(json!({"cancel_requested":true}))
            }
            _ => Err(format!("Unknown tool: {name}")),
        }
    }
}
fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| format!("Invalid tool arguments: {e}"))
}
fn empty(args: Value) -> Result<()> {
    if args == json!({}) {
        Ok(())
    } else {
        Err("Tool takes an empty object".into())
    }
}
