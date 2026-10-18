use blyacut_core::*;
fn editor() -> Editor {
    let mut e = Editor::default();
    e.import(Asset {
        id: "asset".into(),
        name: "test".into(),
        path: "/tmp/test.mp4".into(),
        duration_ms: 10000,
        width: 320,
        height: 180,
        has_audio: true,
    })
    .unwrap();
    e.execute(Edit::Append {
        asset_id: "asset".into(),
        clip_id: "clip".into(),
        track_id: e.project.tracks[0].id.clone(),
    })
    .unwrap();
    e
}
#[test]
fn split_preserves_source_and_timeline() {
    let mut e = editor();
    e.execute(Edit::SplitClip {
        clip_id: "clip".into(),
        position_ms: 3500,
        new_clip_id: "right".into(),
    })
    .unwrap();
    let c = &e.project.tracks[0].clips;
    assert_eq!(c[0].source_end_ms, 3500);
    assert_eq!(c[1].source_start_ms, 3500);
    assert_eq!(c[1].position_ms, 3500);
    assert_eq!(e.project.duration(), 10000);
}
#[test]
fn invalid_operations_are_atomic() {
    let mut e = editor();
    let before = e.project.clone();
    for edit in [
        Edit::SplitClip {
            clip_id: "clip".into(),
            position_ms: 0,
            new_clip_id: "right".into(),
        },
        Edit::TrimClip {
            clip_id: "clip".into(),
            start_ms: 1,
            end_ms: 11000,
        },
        Edit::SplitClip {
            clip_id: "clip".into(),
            position_ms: 5000,
            new_clip_id: "asset".into(),
        },
        Edit::DeleteClip {
            clip_id: "unknown".into(),
        },
    ] {
        assert!(e.execute(edit).is_err());
        assert_eq!(e.project, before);
    }
}
#[test]
fn ripple_trim_delete_and_history() {
    let mut e = editor();
    e.execute(Edit::SplitClip {
        clip_id: "clip".into(),
        position_ms: 4000,
        new_clip_id: "right".into(),
    })
    .unwrap();
    let split = e.project.clone();
    e.execute(Edit::TrimClip {
        clip_id: "clip".into(),
        start_ms: 1000,
        end_ms: 3000,
    })
    .unwrap();
    assert_eq!(e.project.tracks[0].clips[1].position_ms, 2000);
    e.undo();
    assert_eq!(e.project, split);
    e.redo();
    assert_eq!(e.project.duration(), 8000);
    e.execute(Edit::DeleteClip {
        clip_id: "clip".into(),
    })
    .unwrap();
    assert_eq!(e.project.tracks[0].clips[0].position_ms, 0);
    assert_eq!(e.project.duration(), 6000);
}
#[test]
fn rejects_overlap_and_unknown_version() {
    let mut e = editor();
    let mut duplicate = e.project.tracks[0].clips[0].clone();
    duplicate.id = "overlap".into();
    e.project.tracks[0].clips.push(duplicate);
    assert!(e.project.validate().is_err());
    let p = Project {
        version: 99,
        ..Project::default()
    };
    assert!(p.validate().is_err());
}
#[test]
fn command_json_is_strict() {
    assert!(serde_json::from_str::<Edit>(r#"{"operation":"run_shell","command":"bad"}"#).is_err());
    assert!(serde_json::from_str::<Edit>(
        r#"{"operation":"delete_clip","clip_id":"clip","extra":true}"#
    )
    .is_err());
    assert!(serde_json::from_str::<Edit>(
        r#"{"operation":"split_clip","clip_id":"clip","position_ms":-1,"new_clip_id":"x"}"#
    )
    .is_err());
}
#[test]
fn deterministic_replay_and_reorder() {
    let mut e = editor();
    let mut other = Editor::new(e.project.clone()).unwrap();
    let commands = [
        Edit::SplitClip {
            clip_id: "clip".into(),
            position_ms: 4000,
            new_clip_id: "right".into(),
        },
        Edit::ReorderClip {
            clip_id: "right".into(),
            index: 0,
        },
    ];
    for c in commands {
        e.execute(c.clone()).unwrap();
        other.execute(c).unwrap();
    }
    assert_eq!(e.project, other.project);
    assert_eq!(e.project.tracks[0].clips[0].id, "right");
    assert_eq!(e.project.tracks[0].clips[1].position_ms, 6000);
}
#[test]
fn portable_serialization_roundtrip() {
    let mut e = editor();
    let dir = std::env::temp_dir().join(id());
    std::fs::create_dir(&dir).unwrap();
    e.project.assets[0].path = dir.join("test.mp4");
    let path = dir.join("test.blyacut");
    save(&e.project, &path).unwrap();
    let json = std::fs::read_to_string(&path).unwrap();
    assert!(json.contains("\"path\": \"test.mp4\""));
    assert_eq!(load(&path).unwrap(), e.project);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn project_save_cannot_overwrite_video() {
    let e = editor();
    let path = std::env::temp_dir().join(format!("{}.mp4", id()));
    std::fs::write(&path, b"original video bytes").unwrap();
    assert!(save(&e.project, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"original video bytes");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn audio_tracks_keep_positions_and_batch_is_one_undo() {
    let mut e = editor();
    let original = e.project.clone();
    let revision = e.revision;
    let commands = [
        Edit::AddTrack {
            track_id: "music".into(),
            name: "Music".into(),
            kind: TrackKind::Audio,
        },
        Edit::Append {
            track_id: "music".into(),
            asset_id: "asset".into(),
            clip_id: "song".into(),
        },
        Edit::TrimClip {
            clip_id: "song".into(),
            start_ms: 1000,
            end_ms: 5000,
        },
        Edit::MoveClip {
            clip_id: "song".into(),
            track_id: "music".into(),
            position_ms: 2000,
        },
        Edit::SetAudio {
            clip_id: "song".into(),
            gain_milli: 250,
            fade_in_ms: 500,
            fade_out_ms: 1000,
        },
    ];
    let preview = e.preview(&commands).unwrap();
    assert_eq!(e.project, original);
    assert_eq!(e.revision, revision);
    e.batch(&commands, revision).unwrap();
    assert_eq!(e.project, preview);
    assert_eq!(e.revision, revision + 1);
    e.execute(Edit::TrimClip {
        clip_id: "song".into(),
        start_ms: 1500,
        end_ms: 4500,
    })
    .unwrap();
    assert_eq!(e.project.tracks.last().unwrap().clips[0].position_ms, 2000);
    e.undo();
    e.undo();
    assert_eq!(e.project, original);
    assert!(e.revision > revision + 1);
    e.redo();
    assert_eq!(e.project, preview);
}
#[test]
fn batch_failure_stale_revision_and_incompatible_media_are_atomic() {
    let mut e = editor();
    let before = e.project.clone();
    let revision = e.revision;
    assert!(e
        .batch(
            &[Edit::DeleteClip {
                clip_id: "clip".into()
            }],
            revision + 1
        )
        .is_err());
    assert!(e
        .batch(
            &[
                Edit::DeleteClip {
                    clip_id: "clip".into()
                },
                Edit::DeleteClip {
                    clip_id: "missing".into()
                }
            ],
            revision
        )
        .is_err());
    assert_eq!(e.project, before);
    assert_eq!(e.revision, revision);
    e.import(Asset {
        id: "audio".into(),
        name: "audio.wav".into(),
        path: "/tmp/audio.wav".into(),
        duration_ms: 1000,
        width: 0,
        height: 0,
        has_audio: true,
    })
    .unwrap();
    assert!(e
        .execute(Edit::Append {
            asset_id: "audio".into(),
            clip_id: "invalid".into(),
            track_id: e.project.tracks[0].id.clone()
        })
        .is_err());
    assert!(e
        .execute(Edit::SetAudio {
            clip_id: "clip".into(),
            gain_milli: 1001,
            fade_in_ms: 0,
            fade_out_ms: 0
        })
        .is_err());
    assert!(e
        .execute(Edit::SetAudio {
            clip_id: "clip".into(),
            gain_milli: 500,
            fade_in_ms: 6000,
            fade_out_ms: 6000
        })
        .is_err());
}
#[test]
fn v1_project_migrates_without_changing_clip_ranges() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/first-cut.blyacut")
        .canonicalize()
        .unwrap();
    let p = load(&path).unwrap();
    assert_eq!(p.version, 3);
    assert_eq!(p.tracks[0].clips[1].source_start_ms, 4000);
    assert_eq!(p.tracks[0].clips[0].gain_milli, 1000);
    assert_eq!(p.tracks[0].clips[0].fade_in_ms, 0);
}
