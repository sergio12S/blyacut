//! Reloadable recipe catalog. Definitions are validated data, with no executable code.
use blyacut_core::{effects::EffectRecipe, Result};
use serde::Serialize;
use std::{io::Write, path::Path};
#[derive(Serialize)]
pub struct Catalog {
    pub recipes: Vec<EffectRecipe>,
    pub warnings: Vec<String>,
}
pub fn load(directory: &Path) -> Result<Catalog> {
    let root = directory.canonicalize().map_err(|e| e.to_string())?;
    if !root.is_dir() {
        return Err("Select an effect recipe directory".into());
    }
    let mut files = std::fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .take(129)
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    if files.len() > 128 {
        return Err("Effect directory exceeds 128 entries".into());
    }
    files.sort_by_key(|f| f.file_name());
    let mut catalog = Catalog {
        recipes: vec![],
        warnings: vec![],
    };
    let mut ids = std::collections::HashSet::new();
    for file in files {
        let path = file.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let read = (|| -> Result<EffectRecipe> {
            let canonical = path.canonicalize().map_err(|e| e.to_string())?;
            let metadata = std::fs::metadata(&canonical).map_err(|e| e.to_string())?;
            if !canonical.starts_with(&root) || !metadata.is_file() || metadata.len() > 65536 {
                return Err("Recipe must be a local regular JSON file under 64 KiB".into());
            }
            let bytes = std::fs::read(&canonical).map_err(|e| e.to_string())?;
            let recipe: EffectRecipe = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            recipe.validate()?;
            if !ids.insert(recipe.id.clone()) {
                return Err("Duplicate recipe ID".into());
            }
            Ok(recipe)
        })();
        match read {
            Ok(recipe) => catalog.recipes.push(recipe),
            Err(error) => {
                catalog
                    .warnings
                    .push(format!("{}: {}", file.file_name().to_string_lossy(), error))
            }
        }
    }
    Ok(catalog)
}
pub fn create(directory: &Path, recipe: &EffectRecipe) -> Result<()> {
    recipe.validate()?;
    let bytes = serde_json::to_vec_pretty(recipe).map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("Recipe exceeds 64 KiB".into());
    }
    let path = directory.join(format!("{}.json", recipe.id));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    if let Err(error) = file.write_all(&bytes) {
        let _ = std::fs::remove_file(path);
        return Err(error.to_string());
    }
    Ok(())
}
