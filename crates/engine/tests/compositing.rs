use blyacut_core::*;
use blyacut_engine::{studio::*, *};
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
/// RGB of one pixel at fractional (x, y), from a video at `t` seconds or from a still.
fn pixel(path: &std::path::Path, t: Option<f64>, x: f64, y: f64) -> [u8; 3] {
    let mut cmd = Command::new(binary("ffmpeg"));
    cmd.args(["-v", "error"]);
    if let Some(t) = t {
        cmd.args(["-ss", &t.to_string()]);
    }
    let out = cmd
        .arg("-i")
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
fn near(p: [u8; 3], want: [u8; 3]) -> bool {
    p.iter().zip(want).all(|(a, b)| a.abs_diff(b) < 60)
}

#[test]
fn overlays_picture_in_picture_text_and_stills_composite_in_export_and_preview() {
    let dir = std::env::temp_dir().join(format!("blyacut-comp-{}", id()));
    std::fs::create_dir(&dir).unwrap();
    let p = |n: &str| dir.join(n);
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=red:s=640x360:r=30",
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
        p("red.mp4").to_str().unwrap(),
    ]);
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:s=640x360:r=30",
        "-t",
        "3",
        "-c:v",
        "libx264",
        p("blue.mp4").to_str().unwrap(),
    ]);
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=green:s=320x180",
        "-frames:v",
        "1",
        p("logo.png").to_str().unwrap(),
    ]);

    let mut e = Editor::default();
    e.project.settings.width = 640;
    e.project.settings.height = 360;
    text_overlay(&e.project, "ПРИВЕТ", [255, 255, 255], 0.8, &p("text.png")).unwrap();
    let mut ids = vec![];
    for name in ["red.mp4", "blue.mp4", "logo.png", "text.png"] {
        let a = probe(&p(name)).unwrap();
        ids.push(a.id.clone());
        e.import(a).unwrap();
    }
    assert!(e.project.assets[2].still && e.project.assets[2].duration_ms == STILL_MS);
    let main = e.project.tracks[0].id.clone();
    e.batch(
        &[
            Edit::AddTrack {
                track_id: "v2".into(),
                name: "Video 2".into(),
                kind: TrackKind::Video,
            },
            Edit::AddTrack {
                track_id: "v3".into(),
                name: "Video 3".into(),
                kind: TrackKind::Video,
            },
            Edit::Append {
                asset_id: ids[0].clone(),
                clip_id: "red".into(),
                track_id: main.clone(),
            },
            Edit::Append {
                asset_id: ids[2].clone(),
                clip_id: "logo".into(),
                track_id: main.clone(),
            },
            Edit::TrimClip {
                clip_id: "logo".into(),
                start_ms: 0,
                end_ms: 1000,
            },
            // Blue picture-in-picture: 30% size in the bottom-right corner from 1 s to 2 s.
            Edit::Append {
                asset_id: ids[1].clone(),
                clip_id: "pip".into(),
                track_id: "v2".into(),
            },
            Edit::TrimClip {
                clip_id: "pip".into(),
                start_ms: 0,
                end_ms: 1000,
            },
            Edit::MoveClip {
                clip_id: "pip".into(),
                track_id: "v2".into(),
                position_ms: 1000,
            },
            Edit::SetTransform {
                clip_id: "pip".into(),
                transform: Some(Transform {
                    x_milli: 800,
                    y_milli: 780,
                    scale_milli: 300,
                    opacity_milli: 1000,
                }),
            },
            // Text over everything from 0 to 4 s.
            Edit::Append {
                asset_id: ids[3].clone(),
                clip_id: "text".into(),
                track_id: "v3".into(),
            },
            Edit::TrimClip {
                clip_id: "text".into(),
                start_ms: 0,
                end_ms: 4000,
            },
        ],
        e.revision,
    )
    .unwrap();
    // Overlay tracks never ripple: trimming the PiP keeps it at 1 s.
    assert_eq!(e.project.tracks[2].clips[0].position_ms, 1000);
    assert!(
        e.execute(Edit::SetTransform {
            clip_id: "red".into(),
            transform: Some(Transform::default())
        })
        .is_err(),
        "main track has no transform"
    );
    assert!(!e
        .project
        .warnings()
        .iter()
        .any(|w| w.contains("without sound")));

    let out = p("comp.mp4");
    render(&e.project, &out, Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    assert!(probe(&out).unwrap().duration_ms.abs_diff(4000) < 120);
    // 0.5 s: red main, no PiP; corner red.
    assert!(
        near(pixel(&out, Some(0.5), 0.8, 0.78), [255, 0, 0]),
        "{:?}",
        pixel(&out, Some(0.5), 0.8, 0.78)
    );
    // 1.5 s: PiP blue in the corner, red elsewhere.
    assert!(
        near(pixel(&out, Some(1.5), 0.8, 0.78), [0, 0, 255]),
        "{:?}",
        pixel(&out, Some(1.5), 0.8, 0.78)
    );
    assert!(near(pixel(&out, Some(1.5), 0.2, 0.3), [255, 0, 0]));
    // 2.5 s: PiP gone again.
    assert!(near(pixel(&out, Some(2.5), 0.8, 0.78), [255, 0, 0]));
    // 3.5 s: the still logo (green, letterboxed) is the main clip.
    assert!(
        near(pixel(&out, Some(3.5), 0.5, 0.3), [0, 128, 0]),
        "{:?}",
        pixel(&out, Some(3.5), 0.5, 0.3)
    );
    // Text: some white near 80% height while the top stays red.
    let white = (10..90).any(|i| {
        pixel(&out, Some(0.5), i as f64 / 100.0, 0.8)
            .iter()
            .all(|c| *c > 200)
    });
    assert!(white, "text overlay missing");

    // Preview matches export at 1.5 s.
    let frame = p("frame.png");
    preview_frame(&e.project, 1500, &frame, 640).unwrap();
    assert!(near(pixel(&frame, None, 0.8, 0.78), [0, 0, 255]));
    assert!(near(pixel(&frame, None, 0.2, 0.3), [255, 0, 0]));
    if std::env::var_os("BLYACUT_KEEP").is_none() {
        std::fs::remove_dir_all(dir).unwrap();
    } else {
        eprintln!("kept {}", dir.display());
    }
}
