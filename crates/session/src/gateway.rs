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
