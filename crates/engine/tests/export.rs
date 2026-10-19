use blyacut_core::*;
use blyacut_engine::*;
use std::{
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};
#[test]
fn arguments_keep_paths_as_one_argument() {
    let p = Project::default();
    let a = Asset {
        id: id(),
        name: "test".into(),
        path: "/tmp/a ' ; $(touch nope).mp4".into(),
        duration_ms: 5000,
        width: 320,
        height: 180,
        has_audio: false,
        still: false,
    };
    let args = segment_args(&p, &a, 1234, 2500, std::path::Path::new("/tmp/out.mp4"));
    assert!(args.contains(&a.path.to_string_lossy().to_string()));
    assert!(args.contains(&"1.234".into()));
    assert!(args.contains(&"2.500".into()));
    assert!(args.contains(&"anullsrc=r=48000:cl=stereo".into()));
}
#[test]
fn real_export_cut_save_load_probe_and_cancel() {
    assert!(
        Command::new(binary("ffmpeg"))
            .arg("-version")
            .output()
            .is_ok(),
        "FFmpeg is required for integration tests"
    );
    let dir = std::env::temp_dir().join(format!("blyacut-test-{}", id()));
    std::fs::create_dir(&dir).unwrap();
    for audio in [true, false] {
        let input = dir.join(if audio {
            "input audio.mp4"
        } else {
            "input silent.mp4"
        });
        let mut cmd = Command::new(binary("ffmpeg"));
        cmd.args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=30",
        ]);
        if audio {
            cmd.args(["-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000"]);
        }
        cmd.args([
            "-t", "3", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac",
        ])
        .arg(&input);
        assert!(cmd.status().unwrap().success());
        let a = probe(&input).unwrap();
        assert_eq!(a.has_audio, audio);
        let mut e = Editor::default();
        e.project.settings.width = 320;
        e.project.settings.height = 180;
        let aid = a.id.clone();
        e.import(a).unwrap();
        e.execute(Edit::Append {
            asset_id: aid,
            clip_id: "a".into(),
            track_id: e.project.tracks[0].id.clone(),
        })
        .unwrap();
        e.execute(Edit::SplitClip {
            clip_id: "a".into(),
            position_ms: 1000,
            new_clip_id: "b".into(),
        })
        .unwrap();
        e.execute(Edit::SplitClip {
            clip_id: "b".into(),
            position_ms: 2000,
            new_clip_id: "c".into(),
        })
        .unwrap();
        e.execute(Edit::DeleteClip {
            clip_id: "b".into(),
        })
        .unwrap();
        let project = dir.join("test.blyacut");
        save(&e.project, &project).unwrap();
        let p = load(&project).unwrap();
        let out = dir.join(if audio {
            "output.mp4"
        } else {
            "silent-output.mp4"
        });
        let mut progress = vec![];
        render(&p, &out, Arc::new(AtomicBool::new(false)), |v| {
            progress.push(v)
        })
        .unwrap();
        let result = probe(&out).unwrap();
        assert_eq!((result.width, result.height), (320, 180));
        assert!(
            result.duration_ms.abs_diff(2000) < 100,
            "duration {}",
            result.duration_ms
        );
        assert!(result.has_audio);
        assert_eq!(progress.last(), Some(&1.0));
        assert!(render(&p, &out, Arc::new(AtomicBool::new(false)), |_| {}).is_err());
        let cancelled = dir.join("cancelled.mp4");
        assert!(
            render(&p, &cancelled, Arc::new(AtomicBool::new(true)), |_| {})
                .unwrap_err()
                .contains("cancelled")
        );
        assert!(!cancelled.exists());
    }
    assert!(!std::fs::read_dir(&dir).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".blyacut-render")));
    std::fs::remove_dir_all(dir).unwrap();
}
