use blyacut_core::{effects::*, *};
use blyacut_engine::*;
use std::{
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};

fn clip() -> Clip {
    Clip {
        id: "c".into(),
        asset_id: "a".into(),
        position_ms: 0,
        source_start_ms: 0,
        source_end_ms: 1000,
        gain_milli: 1000,
        fade_in_ms: 0,
        fade_out_ms: 0,
        effects: vec![],
        transform: None,
    }
}

fn ffmpeg_ok(args: &[&str]) -> Result<()> {
    let out = Command::new(binary("ffmpeg"))
        .args(["-v", "error", "-nostdin"])
        .args(args)
        .output()
        .unwrap();
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into())
    }
}

/// Every operator FFmpeg is given must parse and run, both as -vf/-af and inside -filter_complex.
#[test]
fn every_effect_step_compiles_to_a_working_ffmpeg_filter() {
    let mut failures = vec![];
    for (name, video, ranges) in SPEC {
        for pick in [0, 1] {
            let mut step = serde_json::json!({ "effect": name });
            for (param, min, max) in ranges.iter() {
                step[*param] = serde_json::json!(if pick == 0 { *min } else { *max });
            }
            let step: EffectStep = serde_json::from_value(step).unwrap();
            let filter = step_filter(&step, &clip());
            let result = if *video {
                ffmpeg_ok(&[
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=s=320x180:r=30:d=0.5",
                    "-vf",
                    &format!("format=yuv420p,{filter},format=yuv420p"),
                    "-f",
                    "null",
                    "-",
                ])
            } else {
                ffmpeg_ok(&[
                    "-f", "lavfi", "-i", "sine=frequency=330:sample_rate=48000:duration=0.5",
                    "-af", &format!("aresample=48000,apad,{filter}"),
                    "-t", "0.5", "-f", "null", "-",
                ])
                .and_then(|_| {
                    ffmpeg_ok(&[
                        "-f", "lavfi", "-i", "sine=frequency=330:sample_rate=44100:duration=0.5",
                        "-filter_complex",
                        &format!("[0:a]asetpts=PTS-STARTPTS,aresample=48000,aformat=channel_layouts=stereo,{filter}[out]"),
                        "-map", "[out]", "-f", "null", "-",
                    ])
                })
            };
            if let Err(e) = result {
                failures.push(format!("{name} ({filter}): {e}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn builtin_presets_render_end_to_end_and_keep_durations() {
    let dir = std::env::temp_dir().join(format!("blyacut-fxpack-{}", id()));
    std::fs::create_dir(&dir).unwrap();
    let video = dir.join("video.mp4");
    let voice = dir.join("voice.wav");
    ffmpeg_ok(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=red:s=320x180:r=30",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=220:sample_rate=48000",
        "-t",
        "2",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        video.to_str().unwrap(),
    ])
    .unwrap();
    ffmpeg_ok(&[
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=44100",
        "-t",
        "1.5",
        voice.to_str().unwrap(),
    ])
    .unwrap();
    let preset = |id: &str| {
        builtin_recipes()
            .into_iter()
            .find(|r| r.id == format!("builtin-{id}"))
            .unwrap()
    };
    let mut e = Editor::default();
    e.project.settings.width = 320;
    e.project.settings.height = 180;
    let v = probe(&video).unwrap();
    let a = probe(&voice).unwrap();
    let (vid, aid) = (v.id.clone(), a.id.clone());
    e.import(v).unwrap();
    e.import(a).unwrap();
    let (vt, at) = (
        e.project.tracks[0].id.clone(),
        e.project.tracks[1].id.clone(),
    );
    e.batch(
        &[
            Edit::Append {
                asset_id: vid,
                clip_id: "v".into(),
                track_id: vt,
            },
            Edit::Append {
                asset_id: aid,
                clip_id: "a".into(),
                track_id: at,
            },
            Edit::SetEffects {
                clip_id: "v".into(),
                effects: vec![
                    preset("vhs"),
                    preset("negative"),
                    preset("fade-in"),
                    preset("demon"),
                ],
            },
            Edit::SetEffects {
                clip_id: "a".into(),
                effects: vec![preset("robot"), preset("chipmunk"), preset("cathedral")],
            },
        ],
        e.revision,
    )
    .unwrap();
    let out = dir.join("fx.mp4");
    render(&e.project, &out, Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    assert!(probe(&out).unwrap().duration_ms.abs_diff(2000) < 100);
    // Red source inverted -> cyan after the 1 s fade-in has finished.
    let frame = Command::new(binary("ffmpeg"))
        .args(["-v", "error", "-ss", "1.5", "-i"])
        .arg(&out)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "scale=1:1,format=rgb24",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .unwrap();
    let p = &frame.stdout[..3];
    assert!(p[0] < 90 && p[1] > 150 && p[2] > 150, "not inverted: {p:?}");
    // Audio clips cannot carry video presets.
    assert!(e
        .execute(Edit::SetEffects {
            clip_id: "a".into(),
            effects: vec![preset("noir")]
        })
        .is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
