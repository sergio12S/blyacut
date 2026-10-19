use blyacut_core::*;
use blyacut_engine::*;
use std::{
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};

fn ffmpeg(args: &[&str]) {
    assert!(Command::new(binary("ffmpeg"))
        .args(["-v", "error"])
        .args(args)
        .status()
        .unwrap()
        .success());
}
fn pixel(path: &std::path::Path, t: f64, x: f64, y: f64) -> [u8; 3] {
    let out = Command::new(binary("ffmpeg"))
        .args(["-v", "error", "-ss", &t.to_string(), "-i"])
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            &format!("crop=2:2:iw*{x}:ih*{y},scale=1:1,format=rgb24"),
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .unwrap();
    [out.stdout[0], out.stdout[1], out.stdout[2]]
}
fn color(dir: &std::path::Path, name: &str, c: &str, size: &str) -> Asset {
    let path = dir.join(name);
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        &format!("color=c={c}:s={size}:r=30"),
        "-f",
        "lavfi",
        "-i",
        "sine",
        "-t",
        "3",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        path.to_str().unwrap(),
    ]);
    probe(&path).unwrap()
}

#[test]
fn crossfade_uses_handles_and_keeps_the_timeline_length() {
    let dir = std::env::temp_dir().join(format!("blyacut-tr-{}", id()));
    std::fs::create_dir(&dir).unwrap();
    let mut e = Editor::default();
    e.project.settings.width = 320;
    e.project.settings.height = 180;
    let (red, blue) = (
        color(&dir, "red.mp4", "red", "320x180"),
        color(&dir, "blue.mp4", "blue", "320x180"),
    );
    let (r, b) = (red.id.clone(), blue.id.clone());
    e.import(red).unwrap();
    e.import(blue).unwrap();
    let vt = e.project.tracks[0].id.clone();
    let fade = |ms| {
        Some(Transition {
            kind: TransitionKind::Fade,
            duration_ms: ms,
        })
    };
    e.batch(
        &[
            Edit::Append {
                asset_id: r,
                clip_id: "red".into(),
                track_id: vt.clone(),
            },
            Edit::Append {
                asset_id: b,
                clip_id: "blue".into(),
                track_id: vt,
            },
            Edit::TrimClip {
                clip_id: "red".into(),
                start_ms: 0,
                end_ms: 2000,
            },
        ],
        e.revision,
    )
    .unwrap();
    // The incoming clip starts at source 0: no media before its in point.
    let err = e
        .execute(Edit::SetTransition {
            clip_id: "blue".into(),
            transition: fade(1000),
        })
        .unwrap_err();
    assert!(
        err.contains("500 ms before the incoming clip's start"),
        "{err}"
    );
    e.batch(
        &[
            Edit::TrimClip {
                clip_id: "blue".into(),
                start_ms: 1000,
                end_ms: 3000,
            },
            Edit::SetTransition {
                clip_id: "blue".into(),
                transition: fade(1000),
            },
        ],
        e.revision,
    )
    .unwrap();
    assert!(
        e.execute(Edit::SetTransition {
            clip_id: "red".into(),
            transition: fade(500)
        })
        .is_err(),
        "first clip"
    );

    let out = dir.join("fade.mp4");
    render(&e.project, &out, Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    assert!(
        probe(&out).unwrap().duration_ms.abs_diff(4000) < 120,
        "timeline length kept"
    );
    let red_px = pixel(&out, 1.0, 0.5, 0.5);
    let mid = pixel(&out, 2.0, 0.5, 0.5);
    let blue_px = pixel(&out, 3.0, 0.5, 0.5);
    assert!(red_px[0] > 200 && red_px[2] < 60, "{red_px:?}");
    assert!(blue_px[2] > 200 && blue_px[0] < 60, "{blue_px:?}");
    assert!(
        mid[0] > 70 && mid[2] > 70 && mid[0] < 200 && mid[2] < 200,
        "not blended: {mid:?}"
    );

    // A wipe and reordering: moving a clip away drops its transition instead of blocking the edit.
    e.execute(Edit::SetTransition {
        clip_id: "blue".into(),
        transition: Some(Transition {
            kind: TransitionKind::Wipeleft,
            duration_ms: 600,
        }),
    })
    .unwrap();
    e.execute(Edit::ReorderClip {
        clip_id: "blue".into(),
        index: 0,
    })
    .unwrap();
    assert!(e.project.tracks[0]
        .clips
        .iter()
        .all(|c| c.transition_in.is_none()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn vertical_fill_crops_instead_of_letterboxing() {
    let dir = std::env::temp_dir().join(format!("blyacut-fill-{}", id()));
    std::fs::create_dir(&dir).unwrap();
    let mut e = Editor::default();
    let wide = color(&dir, "wide.mp4", "green", "640x360");
    let w = wide.id.clone();
    e.import(wide).unwrap();
    let vt = e.project.tracks[0].id.clone();
    e.execute(Edit::Append {
        asset_id: w,
        clip_id: "w".into(),
        track_id: vt,
    })
    .unwrap();
    let vertical = |fill| Settings {
        width: 360,
        height: 640,
        fps_num: 30,
        fps_den: 1,
        fill,
    };
    for fill in [false, true] {
        e.execute(Edit::SetSettings {
            settings: vertical(fill),
        })
        .unwrap();
        let out = dir.join(format!("v-{fill}.mp4"));
        render(&e.project, &out, Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
        let a = probe(&out).unwrap();
        assert_eq!((a.width, a.height), (360, 640));
        let top = pixel(&out, 1.0, 0.5, 0.05);
        if fill {
            assert!(top[1] > 90, "fill should have no bars: {top:?}");
        } else {
            assert!(
                top.iter().all(|c| *c < 20),
                "letterbox bar expected: {top:?}"
            );
        }
    }
    assert!(e
        .execute(Edit::SetSettings {
            settings: Settings {
                width: 361,
                ..vertical(true)
            }
        })
        .is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn every_transition_kind_renders() {
    let dir = std::env::temp_dir().join(format!("blyacut-kinds-{}", id()));
    std::fs::create_dir(&dir).unwrap();
    let mut e = Editor::default();
    e.project.settings.width = 160;
    e.project.settings.height = 90;
    let a = color(&dir, "a.mp4", "red", "160x90");
    // A silent still on the incoming side also exercises the generated-silence path.
    let png = dir.join("b.png");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:s=160x90",
        "-frames:v",
        "1",
        png.to_str().unwrap(),
    ]);
    let b = probe(&png).unwrap();
    let (ai, bi) = (a.id.clone(), b.id.clone());
    e.import(a).unwrap();
    e.import(b).unwrap();
    let vt = e.project.tracks[0].id.clone();
    e.batch(
        &[
            Edit::Append {
                asset_id: ai,
                clip_id: "a".into(),
                track_id: vt.clone(),
            },
            Edit::TrimClip {
                clip_id: "a".into(),
                start_ms: 0,
                end_ms: 1000,
            },
            Edit::Append {
                asset_id: bi,
                clip_id: "b".into(),
                track_id: vt,
            },
            Edit::TrimClip {
                clip_id: "b".into(),
                start_ms: 0,
                end_ms: 1000,
            },
        ],
        e.revision,
    )
    .unwrap();
    for kind in TransitionKind::ALL {
        e.execute(Edit::SetTransition {
            clip_id: "b".into(),
            transition: Some(Transition {
                kind,
                duration_ms: 400,
            }),
        })
        .unwrap();
        let out = dir.join(format!("{}.mp4", kind.name()));
        render(&e.project, &out, Arc::new(AtomicBool::new(false)), |_| {})
            .unwrap_or_else(|err| panic!("{}: {err}", kind.name()));
        assert!(
            probe(&out).unwrap().duration_ms.abs_diff(2000) < 120,
            "{}",
            kind.name()
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}
