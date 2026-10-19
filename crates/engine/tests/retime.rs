use blyacut_core::*;
use blyacut_engine::*;
use std::{
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};

/// A clip whose brightness encodes source time: luma ≈ 16 + t·50 (studio range).
fn ramp(dir: &std::path::Path) -> Asset {
    let path = dir.join("ramp.mp4");
    assert!(Command::new(binary("ffmpeg"))
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=160x90:r=30:d=4,geq=lum='16+T*50':cb=128:cr=128",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=4",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-shortest"
        ])
        .arg(&path)
        .status()
        .unwrap()
        .success());
    probe(&path).unwrap()
}
fn luma(path: &std::path::Path, t: f64) -> f64 {
    let out = Command::new(binary("ffmpeg"))
        .args(["-v", "error", "-ss", &t.to_string(), "-i"])
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "scale=1:1,format=gray",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .unwrap();
    out.stdout[0] as f64
}
/// Source seconds shown, inverted from the ramp (allowing for codec range conversions).
fn shown(path: &std::path::Path, t: f64) -> f64 {
    (luma(path, t) - 16.0) / 50.0
}
fn render_one(e: &Editor, dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let out = dir.join(name);
    render(&e.project, &out, Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    out
}

#[test]
fn speed_and_reverse_map_timeline_to_source() {
    let dir = std::env::temp_dir().join(format!("blyacut-retime-{}", id()));
    std::fs::create_dir(&dir).unwrap();
    let mut e = Editor::default();
    e.project.settings.width = 160;
    e.project.settings.height = 90;
    let a = ramp(&dir);
    let aid = a.id.clone();
    e.import(a).unwrap();
    let vt = e.project.tracks[0].id.clone();
    e.execute(Edit::Append {
        asset_id: aid,
        clip_id: "c".into(),
        track_id: vt,
    })
    .unwrap();
    let base = shown(&render_one(&e, &dir, "normal.mp4"), 2.0);
    assert!((base - 2.0).abs() < 0.25, "ramp calibration: {base}");

    e.execute(Edit::SetSpeed {
        clip_id: "c".into(),
        speed_milli: 2000,
        reverse: false,
    })
    .unwrap();
    assert_eq!(e.project.duration(), 2000);
    let fast = render_one(&e, &dir, "fast.mp4");
    assert!(probe(&fast).unwrap().duration_ms.abs_diff(2000) < 120);
    assert!(
        (shown(&fast, 1.0) - 2.0).abs() < 0.3,
        "2x at 1 s shows source 2 s: {}",
        shown(&fast, 1.0)
    );

    e.execute(Edit::SetSpeed {
        clip_id: "c".into(),
        speed_milli: 500,
        reverse: false,
    })
    .unwrap();
    assert_eq!(e.project.duration(), 8000);
    let slow = render_one(&e, &dir, "slow.mp4");
    assert!(probe(&slow).unwrap().duration_ms.abs_diff(8000) < 150);
    assert!((shown(&slow, 4.0) - 2.0).abs() < 0.3);

    e.execute(Edit::SetSpeed {
        clip_id: "c".into(),
        speed_milli: 1000,
        reverse: true,
    })
    .unwrap();
    let back = render_one(&e, &dir, "reverse.mp4");
    assert!(
        (shown(&back, 0.5) - 3.5).abs() < 0.3,
        "reverse starts at the end: {}",
        shown(&back, 0.5)
    );
    assert!((shown(&back, 3.5) - 0.5).abs() < 0.3);
    // preview_frame agrees with export for a reversed clip.
    let png = dir.join("rev.png");
    studio::preview_frame(&e.project, 500, &png, 160).unwrap();
    assert!(((luma(&png, 0.0) - 16.0) / 50.0 - 3.5).abs() < 0.3);

    // Splitting a reversed clip keeps the order of what is shown.
    e.execute(Edit::SplitClip {
        clip_id: "c".into(),
        position_ms: 1000,
        new_clip_id: "d".into(),
    })
    .unwrap();
    let clips = &e.project.tracks[0].clips;
    assert_eq!(
        (clips[0].source_start_ms, clips[0].source_end_ms),
        (3000, 4000)
    );
    assert_eq!(
        (clips[1].source_start_ms, clips[1].source_end_ms),
        (0, 3000)
    );
    assert!(e
        .execute(Edit::SetSpeed {
            clip_id: "c".into(),
            speed_milli: 5000,
            reverse: false
        })
        .is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
