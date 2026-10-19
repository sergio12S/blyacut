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
/// One row of RGB pixels at height fraction `y`, from a video at `t` (or a still with t = None).
fn row(path: &std::path::Path, t: Option<f64>, y: f64) -> Vec<[u8; 3]> {
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
            &format!("crop=iw:1:0:ih*{y},format=rgb24"),
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .unwrap();
    out.stdout.as_chunks::<3>().0.to_vec()
}
/// Fraction of the row that is mostly blue, and the centre of that run.
fn blue_span(px: &[[u8; 3]]) -> (f64, f64) {
    let blue: Vec<usize> = px
        .iter()
        .enumerate()
        .filter(|(_, p)| p[2] > 120 && p[0] < 140)
        .map(|(i, _)| i)
        .collect();
    if blue.is_empty() {
        return (0.0, -1.0);
    }
    let n = px.len() as f64;
    (
        blue.len() as f64 / n,
        (blue[0] + blue[blue.len() - 1]) as f64 / 2.0 / n,
    )
}

#[test]
fn keyframes_slide_scale_and_fade_an_overlay() {
    let dir = std::env::temp_dir().join(format!("blyacut-keys-{}", id()));
    std::fs::create_dir(&dir).unwrap();
    let p = |n: &str| dir.join(n);
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=red:s=320x180:r=30",
        "-t",
        "2",
        "-c:v",
        "libx264",
        p("red.mp4").to_str().unwrap(),
    ]);
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:s=320x180",
        "-frames:v",
        "1",
        p("blue.png").to_str().unwrap(),
    ]);
    let mut e = Editor::default();
    e.project.settings.width = 320;
    e.project.settings.height = 180;
    let (r, b) = (
        probe(&p("red.mp4")).unwrap(),
        probe(&p("blue.png")).unwrap(),
    );
    let (ri, bi) = (r.id.clone(), b.id.clone());
    e.import(r).unwrap();
    e.import(b).unwrap();
    let vt = e.project.tracks[0].id.clone();
    let key = |at_ms, x, scale, opacity| Keyframe {
        at_ms,
        transform: Transform {
            x_milli: x,
            y_milli: 500,
            scale_milli: scale,
            opacity_milli: opacity,
        },
    };
    e.batch(
        &[
            Edit::AddTrack {
                track_id: "o".into(),
                name: "Video 2".into(),
                kind: TrackKind::Video,
            },
            Edit::Append {
                asset_id: ri,
                clip_id: "red".into(),
                track_id: vt,
            },
            Edit::Append {
                asset_id: bi,
                clip_id: "box".into(),
                track_id: "o".into(),
            },
            Edit::TrimClip {
                clip_id: "box".into(),
                start_ms: 0,
                end_ms: 2000,
            },
            Edit::SetKeyframes {
                clip_id: "box".into(),
                keyframes: vec![key(0, -200, 300, 0), key(1000, 500, 600, 1000)],
            },
        ],
        e.revision,
    )
    .unwrap();
    assert!(
        e.execute(Edit::SetKeyframes {
            clip_id: "red".into(),
            keyframes: vec![key(0, 500, 1000, 1000)]
        })
        .is_err(),
        "main track"
    );
    assert!(
        e.execute(Edit::SetKeyframes {
            clip_id: "box".into(),
            keyframes: vec![key(500, 0, 300, 0), key(500, 0, 300, 0)]
        })
        .is_err(),
        "times must increase"
    );
    assert_eq!(e.project.tracks[2].clips[0].transform_at(500).x_milli, 150);

    let out = p("anim.mp4");
    render(&e.project, &out, Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    // Start: off-frame and transparent.
    let (w0, _) = blue_span(&row(&out, Some(0.05), 0.5));
    assert!(w0 < 0.02, "hidden at the start: {w0}");
    // Middle: half-way left, partly visible; opacity ~0.5 blends to purple.
    let mid = row(&out, Some(0.5), 0.5);
    let purple = mid.iter().filter(|p| p[0] > 70 && p[2] > 70).count() as f64 / mid.len() as f64;
    assert!(purple > 0.15, "half-transparent box mid-move: {purple}");
    // End: centred, 60% of the frame width, opaque.
    let (w1, c1) = blue_span(&row(&out, Some(1.5), 0.5));
    assert!(
        (w1 - 0.6).abs() < 0.06 && (c1 - 0.5).abs() < 0.04,
        "end pose: width {w1}, centre {c1}"
    );
    // preview_frame shows the same end pose.
    let frame = p("end.png");
    preview_frame(&e.project, 1500, &frame, 320).unwrap();
    let (pw, pc) = blue_span(&row(&frame, None, 0.5));
    assert!((pw - w1).abs() < 0.04 && (pc - c1).abs() < 0.03);

    // Splitting keeps the motion continuous across the cut.
    e.execute(Edit::SplitClip {
        clip_id: "box".into(),
        position_ms: 500,
        new_clip_id: "box2".into(),
    })
    .unwrap();
    let parts = &e.project.tracks[2].clips;
    assert_eq!(parts[0].transform_at(500), parts[1].transform_at(0));
    assert_eq!(parts[1].transform_at(500).x_milli, 500);
    std::fs::remove_dir_all(dir).unwrap();
}
