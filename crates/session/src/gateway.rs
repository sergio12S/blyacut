//! The agent adapter limits filesystem access and requires human review for removals/large batches.
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
#[derive(Clone, Serialize)]
pub struct GatewayInfo {
    pub allowed_root: PathBuf,
}
pub struct Gateway {
    pub session: Arc<Session>,
    root: PathBuf,
}
impl Gateway {
    pub fn new(session: Arc<Session>, root: &Path) -> Result<Self> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        if !root.is_dir() {
            return Err("Choose a directory for agent access".into());
        }
        Ok(Self { session, root })
    }
    pub fn info(&self) -> GatewayInfo {
        GatewayInfo {
            allowed_root: self.root.clone(),
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
        if !path.is_absolute() || path.exists() {
            return Err("Export needs an absolute, new filename".into());
        }
        let parent = self.input(path.parent().ok_or("Missing output directory")?)?;
        let name = path.file_name().ok_or("Missing output filename")?;
        if path.extension().and_then(|s| s.to_str()) != Some("mp4") {
            return Err("Output must end in .mp4".into());
        }
        Ok(parent.join(name))
    }
    pub fn call(&self, name: &str, args: Value) -> Result<Value> {
        match name {
            "list_effect_recipes" => {
                empty(args)?;
                let directory = self.root.join("effects");
                if !directory.exists() {
                    return Ok(json!({"recipes":[],"warnings":[]}));
                }
                let directory = self.input(&directory)?;
                Ok(json!(crate::effects::load(&directory)?))
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
                    json!({"revision":s.revision,"tracks":s.project.tracks,"duration_ms":s.project.duration()}),
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
                Ok(json!(self.session.import_at(asset, revision)?))
            }
            "apply_edits" => {
                let b: Batch = parse(args)?;
                if b.edits.len() > 20
                    || b.edits.iter().any(|e| matches!(e, Edit::DeleteClip { .. }))
                {
                    return Err(
                        "Removal or batch >20 edits requires propose_edits and desktop approval"
                            .into(),
                    );
                }
                Ok(json!(self.session.apply(&b.edits, b.expected_revision)?))
            }
            "propose_edits" => {
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
