#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use blyacut_core::{Edit, Result};
#[cfg(unix)]
use blyacut_session::{
    gateway::Gateway,
    ipc::{ConnectionInfo, Server},
};
use blyacut_session::{Event, RenderJob, Session, Snapshot};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager, State};
struct Desktop {
    session: Arc<Session>,
    #[cfg(unix)]
    agent: Mutex<Option<Server>>,
}
#[tauri::command]
fn get_project(s: State<Desktop>) -> Result<Snapshot> {
    s.session.snapshot()
}
#[tauri::command]
fn edit(s: State<Desktop>, operation: Edit, expected_revision: u64) -> Result<Snapshot> {
    s.session.apply(&[operation], expected_revision)
}
#[tauri::command]
fn history(s: State<Desktop>, redo: bool, expected_revision: u64) -> Result<Snapshot> {
    s.session.history(redo, Some(expected_revision))
}
#[tauri::command]
async fn import_media(s: State<'_, Desktop>, path: PathBuf) -> Result<Snapshot> {
    let session = s.session.clone();
    let revision = session.snapshot()?.revision;
    tauri::async_runtime::spawn_blocking(move || {
        let asset = blyacut_engine::probe(&path)?;
        session.import_at(asset, revision)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn save_project(s: State<'_, Desktop>, path: PathBuf) -> Result<u64> {
    let snapshot = s.session.snapshot()?;
    tauri::async_runtime::spawn_blocking(move || {
        blyacut_core::save(&snapshot.project, &path)?;
        Ok(snapshot.revision)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn autosave(app: tauri::AppHandle, s: State<'_, Desktop>) -> Result<String> {
    let p = s.session.snapshot()?.project;
    let directory = app.path().app_data_dir().map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let path = directory.join("autosave.blyacut");
        blyacut_core::save(&p, &path)?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn open_project(s: State<'_, Desktop>, path: PathBuf) -> Result<Snapshot> {
    let session = s.session.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let p = blyacut_core::load(&path)?;
        session.replace(p)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn missing_media(s: State<Desktop>) -> Result<Vec<String>> {
    Ok(s.session
        .snapshot()?
        .project
        .assets
        .iter()
        .filter(|a| !a.path.is_file())
        .map(|a| a.name.clone())
        .collect())
}
#[tauri::command]
fn export_video(s: State<Desktop>, path: PathBuf) -> Result<RenderJob> {
    s.session.export(&path, None)
}
#[tauri::command]
fn cancel_export(s: State<Desktop>) -> Result<()> {
    s.session.cancel()
}
#[tauri::command]
async fn load_effect_recipes(root: PathBuf) -> Result<blyacut_session::effects::Catalog> {
    tauri::async_runtime::spawn_blocking(move || blyacut_session::effects::load(&root))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn get_proposal(s: State<Desktop>) -> Result<Option<blyacut_session::Proposal>> {
    s.session.pending()
}
#[tauri::command]
fn resolve_proposal(s: State<Desktop>, proposal_id: String, accept: bool) -> Result<Snapshot> {
    s.session.resolve_proposal(&proposal_id, accept)
}
#[cfg(unix)]
#[tauri::command]
fn start_agent(s: State<Desktop>, root: PathBuf) -> Result<ConnectionInfo> {
    let mut slot = s.agent.lock().map_err(|e| e.to_string())?;
    if slot.is_some() {
        return Err("Stop the current agent session first".into());
    }
    let server = Server::start(Gateway::new(s.session.clone(), &root)?)?;
    let info = server.info.clone();
    *slot = Some(server);
    Ok(info)
}
#[cfg(unix)]
#[tauri::command]
fn get_agent(s: State<Desktop>) -> Result<Option<ConnectionInfo>> {
    Ok(s.agent
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .map(|s| s.info.clone()))
}
#[cfg(unix)]
#[tauri::command]
async fn stop_agent(s: State<'_, Desktop>) -> Result<()> {
    let server = s.agent.lock().map_err(|e| e.to_string())?.take();
    tauri::async_runtime::spawn_blocking(move || drop(server))
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn main() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let session = Arc::new(Session::new(move |event| match event {
                Event::Project(snapshot) => {
                    for asset in &snapshot.project.assets {
                        let _ = handle.asset_protocol_scope().allow_file(&asset.path);
                    }
                    let _ = handle.emit("project-changed", snapshot);
                }
                Event::Render(job) => {
                    let _ = handle.emit("render", job);
                }
                Event::Proposal(proposal) => {
                    let _ = handle.emit("proposal", proposal);
                }
            }));
            app.manage(Desktop {
                session,
                #[cfg(unix)]
                agent: Mutex::new(None),
            });
            Ok(())
        });
    #[cfg(unix)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        get_project,
        edit,
        history,
        import_media,
        save_project,
        autosave,
        open_project,
        missing_media,
        export_video,
        cancel_export,
        load_effect_recipes,
        get_proposal,
        resolve_proposal,
        start_agent,
        stop_agent,
        get_agent
    ]);
    #[cfg(not(unix))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        get_project,
        edit,
        history,
        import_media,
        save_project,
        autosave,
        open_project,
        missing_media,
        export_video,
        cancel_export,
        load_effect_recipes,
        get_proposal,
        resolve_proposal
    ]);
    builder
        .build(tauri::generate_context!())
        .expect("Could not start BlyaCut")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let _ = app.state::<Desktop>().session.cancel();
            }
        });
}
