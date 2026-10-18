use blyacut_core::*;
use blyacut_session::{gateway::Gateway, Event, Session};
use serde_json::json;
use std::sync::{Arc, Mutex};
fn setup() -> (Arc<Session>, String) {
    let s = Arc::new(Session::default());
    s.import(Asset {
        id: "media".into(),
        name: "test".into(),
        path: "/tmp/test.mp4".into(),
        duration_ms: 10000,
        width: 320,
        height: 180,
        has_audio: true,
    })
    .unwrap();
    let track = s.snapshot().unwrap().project.tracks[0].id.clone();
    (s, track)
}
#[test]
fn agent_and_desktop_share_history_and_revision() {
    let events = Arc::new(Mutex::new(vec![]));
    let target = events.clone();
    let s = Arc::new(Session::new(move |event| {
        target.lock().unwrap().push(event)
    }));
    let root = std::env::temp_dir();
    let gateway = Gateway::new(s.clone(), &root).unwrap();
    let revision = s.snapshot().unwrap().revision;
    let reply=gateway.call("apply_edits",json!({"expected_revision":revision,"edits":[{"operation":"add_track","track_id":"agent-track","name":"Music","kind":"audio"},{"operation":"add_track","track_id":"voice","name":"Voice","kind":"audio"}]})).unwrap();
    assert_eq!(reply["revision"], revision + 1);
    assert!(s
        .snapshot()
        .unwrap()
        .project
        .tracks
        .iter()
        .any(|t| t.id == "agent-track"));
    assert!(events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, Event::Project(_))));
    gateway
        .call("undo", json!({"expected_revision":revision+1}))
        .unwrap();
    assert!(!s
        .snapshot()
        .unwrap()
        .project
        .tracks
        .iter()
        .any(|t| t.id == "agent-track"));
    assert!(gateway.call("apply_edits",json!({"expected_revision":revision,"edits":[{"operation":"delete_clip","clip_id":"fake"}]})).is_err());
}
#[test]
fn removals_require_preview_and_human_approval() {
    let (s, track) = setup();
    s.edit(Edit::Append {
        asset_id: "media".into(),
        clip_id: "clip".into(),
        track_id: track,
    })
    .unwrap();
    let gateway = Gateway::new(s.clone(), &std::env::temp_dir()).unwrap();
    let revision = s.snapshot().unwrap().revision;
    let args = json!({"expected_revision":revision,"edits":[{"operation":"delete_clip","clip_id":"clip"}]});
    assert!(gateway
        .call("apply_edits", args.clone())
        .unwrap_err()
        .contains("approval"));
    let preview = gateway.call("propose_edits", args).unwrap();
    assert!(preview["preview"]["tracks"][0]["clips"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(s.snapshot().unwrap().project.tracks[0].clips.len(), 1);
    assert!(gateway
        .call("resolve_proposal", json!({"accept":true}))
        .is_err());
    let id = preview["id"].as_str().unwrap();
    s.resolve_proposal(id, true).unwrap();
    assert_eq!(s.snapshot().unwrap().project.tracks[0].clips.len(), 0);
    s.history(false, None).unwrap();
    assert_eq!(s.snapshot().unwrap().project.tracks[0].clips.len(), 1);
    let revision = s.snapshot().unwrap().revision;
    let preview = s
        .propose(
            vec![Edit::DeleteClip {
                clip_id: "clip".into(),
            }],
            revision,
        )
        .unwrap();
    s.edit(Edit::SetAudio {
        clip_id: "clip".into(),
        gain_milli: 500,
        fade_in_ms: 0,
        fade_out_ms: 0,
    })
    .unwrap();
    assert!(s.resolve_proposal(&preview.id, true).is_err());
}
#[test]
fn gateway_rejects_unknown_fields_operations_and_stale_commands() {
    let (s, _) = setup();
    let g = Gateway::new(s.clone(), &std::env::temp_dir()).unwrap();
    let before = s.snapshot().unwrap();
    for (name, args) in [
        ("get_project", json!({"extra":true})),
        ("import_media", json!({"path":"/tmp/nope","shell":"bad"})),
        (
            "apply_edits",
            json!({"expected_revision":before.revision,"edits":[{"operation":"run_shell"}]}),
        ),
        (
            "apply_edits",
            json!({"expected_revision":before.revision+1,"edits":[{"operation":"add_track","track_id":"bad","name":"bad","kind":"audio"}]}),
        ),
    ] {
        assert!(g.call(name, args).is_err());
    }
    assert_eq!(s.snapshot().unwrap().revision, before.revision);
    assert_eq!(s.snapshot().unwrap().project, before.project);
}
#[cfg(unix)]
#[test]
fn folder_scope_rejects_symlink_escape_and_ipc_is_private_revocable() {
    use blyacut_session::ipc;
    use std::{
        io::{BufReader, Write},
        os::unix::{
            fs::{symlink, PermissionsExt},
            net::UnixStream,
        },
    };
    let dir = std::env::temp_dir().join(id());
    let outside = std::env::temp_dir().join(id());
    std::fs::create_dir(&dir).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("secret.wav"), b"secret").unwrap();
    symlink(&outside, dir.join("escape")).unwrap();
    let (s, _) = setup();
    let g = Gateway::new(s.clone(), &dir).unwrap();
    assert!(g.input(&dir.join("escape/secret.wav")).is_err());
    assert!(g.output(&dir.join("escape/out.mp4")).is_err());
    assert!(g.output(&dir.join("../out.mp4")).is_err());
    let server = ipc::Server::start(g).unwrap();
    let path = server.info.config_path.clone();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    for _ in 0..32 {
        assert!(ipc::call(&path, "get_project", json!({}))
            .unwrap()
            .get("project")
            .is_some());
    }
    let config: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let mut socket = UnixStream::connect(config["socket_path"].as_str().unwrap()).unwrap();
    writeln!(
        socket,
        "{}",
        json!({"token":"wrong","name":"get_project","arguments":{}})
    )
    .unwrap();
    let response = ipc::read_json(&mut BufReader::new(socket), 1024)
        .unwrap()
        .unwrap();
    assert!(response.get("error").is_some());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(ipc::call(&path, "get_project", json!({})).is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    drop(server);
    assert!(!path.exists());
    assert!(ipc::call(&path, "get_project", json!({})).is_err());
    let mut reader = std::io::Cursor::new(b"{\"data\":\"too long\"}\n");
    assert!(ipc::read_json(&mut reader, 8).is_err());
    std::fs::remove_dir_all(dir).unwrap();
    std::fs::remove_dir_all(outside).unwrap();
}

#[test]
fn recipes_hot_reload_and_agent_creation_is_scoped() {
    use blyacut_core::effects::{EffectRecipe, EffectStep};
    let dir = std::env::temp_dir().join(id());
    std::fs::create_dir(&dir).unwrap();
    let (s, _) = setup();
    let g = Gateway::new(s, &dir).unwrap();
    let recipe = EffectRecipe {
        version: 1,
        id: "radio".into(),
        name: "Radio".into(),
        steps: vec![
            EffectStep::HighPass { frequency_hz: 300 },
            EffectStep::LowPass { frequency_hz: 3000 },
        ],
    };
    assert!(g.call("list_effect_recipes", json!({})).unwrap()["recipes"]
        .as_array()
        .unwrap()
        .is_empty());
    g.call("create_effect_recipe", json!({"recipe":recipe}))
        .unwrap();
    assert!(g
        .call("create_effect_recipe", json!({"recipe":recipe}))
        .is_err());
    let first = g.call("list_effect_recipes", json!({})).unwrap();
    assert_eq!(first["recipes"][0]["name"], "Radio");
    let updated = EffectRecipe {
        name: "Telephone".into(),
        ..recipe
    };
    std::fs::write(
        dir.join("effects/radio.json"),
        serde_json::to_vec(&updated).unwrap(),
    )
    .unwrap();
    let second = g.call("list_effect_recipes", json!({})).unwrap();
    assert_eq!(second["recipes"][0]["name"], "Telephone");
    std::fs::write(dir.join("effects/broken.json"), b"not json").unwrap();
    let third = g.call("list_effect_recipes", json!({})).unwrap();
    assert_eq!(third["recipes"].as_array().unwrap().len(), 1);
    assert_eq!(third["warnings"].as_array().unwrap().len(), 1);
    assert!(g.call("create_effect_recipe",json!({"recipe":{"version":1,"id":"../escape","name":"Bad","steps":[{"effect":"hue","degrees":0}]}})).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn pointer_left_trim_batches_preserve_neighbour_and_undo_once() {
    let (s, _) = setup();
    let audio = s.snapshot().unwrap().project.tracks[1].id.clone();
    let edits = [
        Edit::Append {
            asset_id: "media".into(),
            clip_id: "left".into(),
            track_id: audio.clone(),
        },
        Edit::TrimClip {
            clip_id: "left".into(),
            start_ms: 1000,
            end_ms: 3000,
        },
        Edit::MoveClip {
            clip_id: "left".into(),
            track_id: audio.clone(),
            position_ms: 1000,
        },
        Edit::Append {
            asset_id: "media".into(),
            clip_id: "next".into(),
            track_id: audio.clone(),
        },
        Edit::TrimClip {
            clip_id: "next".into(),
            start_ms: 0,
            end_ms: 2000,
        },
    ];
    let before = s.apply(&edits, s.snapshot().unwrap().revision).unwrap();
    // Shrink first so moving right does not transiently collide with the neighbour.
    let shrunk = s
        .apply(
            &[
                Edit::TrimClip {
                    clip_id: "left".into(),
                    start_ms: 2000,
                    end_ms: 3000,
                },
                Edit::MoveClip {
                    clip_id: "left".into(),
                    track_id: audio.clone(),
                    position_ms: 2000,
                },
            ],
            before.revision,
        )
        .unwrap();
    assert_eq!(shrunk.revision, before.revision + 1);
    assert_eq!(shrunk.project.tracks[1].clips[0].position_ms, 2000);
    assert_eq!(shrunk.project.tracks[1].clips[1].position_ms, 3000);
    let restored = s.history(false, Some(shrunk.revision)).unwrap();
    assert_eq!(restored.project, before.project);
    // Extend left by moving first: extending at the original position would overlap.
    let extended = s
        .apply(
            &[
                Edit::MoveClip {
                    clip_id: "left".into(),
                    track_id: audio,
                    position_ms: 500,
                },
                Edit::TrimClip {
                    clip_id: "left".into(),
                    start_ms: 500,
                    end_ms: 3000,
                },
            ],
            restored.revision,
        )
        .unwrap();
    assert_eq!(extended.project.tracks[1].clips[0].position_ms, 500);
    assert_eq!(extended.project.tracks[1].clips[0].duration(), 2500);
    assert_eq!(extended.project.tracks[1].clips[1].position_ms, 3000);
    assert_eq!(
        s.history(false, Some(extended.revision)).unwrap().project,
        before.project
    );
}
