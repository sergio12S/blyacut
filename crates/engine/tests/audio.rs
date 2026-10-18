use blyacut_core::*;
use blyacut_engine::*;
use std::{
    path::Path,
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};
fn fixture(args: &[&str], path: &Path) {
    assert!(Command::new(binary("ffmpeg"))
        .args(["-v", "error"])
        .args(args)
        .arg(path)
        .status()
        .unwrap()
        .success());
}
fn pcm(path: &Path) -> Vec<f32> {
    let output = Command::new(binary("ffmpeg"))
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-vn", "-f", "f32le", "-ac", "1", "-ar", "48000", "pipe:1"])
        .output()
        .unwrap();
    assert!(output.status.success());
    output
        .stdout
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect()
}
fn rms(samples: &[f32], start: f64, end: f64) -> f64 {
    let values = &samples[(start * 48000.0) as usize..(end * 48000.0) as usize];
    (values.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / values.len() as f64).sqrt()
}
#[test]
fn audio_mix_has_expected_delay_gain_fades_and_video_gap() {
    let dir = std::env::temp_dir().join(format!("blyacut-audio-{}", id()));
    std::fs::create_dir(&dir).unwrap();
    let video = dir.join("video.mp4");
    let song = dir.join("song.wav");
    fixture(
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=320x180:r=30",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=220:sample_rate=48000",
            "-t",
            "3",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
        ],
        &video,
    );
    fixture(
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=880:sample_rate=48000",
            "-t",
            "2",
            "-c:a",
            "pcm_s16le",
        ],
        &song,
    );
    let mut e = Editor::default();
    e.project.settings.width = 320;
    e.project.settings.height = 180;
    let va = probe(&video).unwrap();
    let aid = va.id.clone();
    e.import(va).unwrap();
    let song = probe(&song).unwrap();
    let sid = song.id.clone();
    assert_eq!((song.width, song.height), (0, 0));
    e.import(song).unwrap();
    let vt = e.project.tracks[0].id.clone();
    let at = e.project.tracks[1].id.clone();
    e.batch(
        &[
            Edit::Append {
                asset_id: aid,
                clip_id: "v".into(),
                track_id: vt.clone(),
            },
            Edit::SetAudio {
                clip_id: "v".into(),
                gain_milli: 0,
                fade_in_ms: 0,
                fade_out_ms: 0,
            },
            Edit::SplitClip {
                clip_id: "v".into(),
                position_ms: 1000,
                new_clip_id: "v2".into(),
            },
            Edit::TrimClip {
                clip_id: "v2".into(),
                start_ms: 1000,
                end_ms: 2000,
            },
            Edit::MoveClip {
                clip_id: "v2".into(),
                track_id: vt,
                position_ms: 2000,
            },
            Edit::Append {
                asset_id: sid,
                clip_id: "a".into(),
                track_id: at.clone(),
            },
            Edit::TrimClip {
                clip_id: "a".into(),
                start_ms: 250,
                end_ms: 1750,
            },
            Edit::MoveClip {
                clip_id: "a".into(),
                track_id: at,
                position_ms: 500,
            },
            Edit::SetAudio {
                clip_id: "a".into(),
                gain_milli: 500,
                fade_in_ms: 250,
                fade_out_ms: 250,
            },
        ],
        e.revision,
    )
    .unwrap();
    let out = dir.join("mix.mp4");
    render(&e.project, &out, Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    let asset = probe(&out).unwrap();
    assert_eq!((asset.width, asset.height), (320, 180));
    assert!(asset.duration_ms.abs_diff(3000) < 100);
    let samples = pcm(&out);
    let peak = rms(&samples, 1.0, 1.1);
    assert!(peak > 0.02 && peak < 0.06, "mix rms {peak}");
    assert!(rms(&samples, 0.1, 0.2) < 0.001);
    assert!(rms(&samples, 2.3, 2.4) < 0.001);
    assert!(rms(&samples, 0.52, 0.57) < peak * 0.5);
    assert!(rms(&samples, 1.93, 1.98) < peak * 0.5);
    // Inspect an actual frame in the video gap: black instead of a frozen source frame.
    let frame = Command::new(binary("ffmpeg"))
        .args(["-v", "error", "-ss", "1.5", "-i"])
        .arg(&out)
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
    assert!(frame.status.success());
    assert!(frame.stdout[0] < 20);
    e.execute(Edit::SetAudio {
        clip_id: "a".into(),
        gain_milli: 250,
        fade_in_ms: 250,
        fade_out_ms: 250,
    })
    .unwrap();
    let quieter = dir.join("quiet.mp4");
    render(
        &e.project,
        &quieter,
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .unwrap();
    let quiet = rms(&pcm(&quieter), 1.0, 1.1);
    assert!((quiet / peak - 0.5).abs() < 0.08);
    // Two overlapping audio tracks add without automatic normalization.
    e.batch(
        &[
            Edit::AddTrack {
                track_id: "second-audio".into(),
                name: "Second audio".into(),
                kind: TrackKind::Audio,
            },
            Edit::Append {
                asset_id: e.project.assets[1].id.clone(),
                clip_id: "a2".into(),
                track_id: "second-audio".into(),
            },
            Edit::TrimClip {
                clip_id: "a2".into(),
                start_ms: 250,
                end_ms: 1750,
            },
            Edit::MoveClip {
                clip_id: "a2".into(),
                track_id: "second-audio".into(),
                position_ms: 500,
            },
            Edit::SetAudio {
                clip_id: "a2".into(),
                gain_milli: 250,
                fade_in_ms: 250,
                fade_out_ms: 250,
            },
        ],
        e.revision,
    )
    .unwrap();
    let doubled = dir.join("two-tracks.mp4");
    render(
        &e.project,
        &doubled,
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .unwrap();
    let both = rms(&pcm(&doubled), 1.0, 1.1);
    assert!((both / peak - 1.0).abs() < 0.08);
    use blyacut_core::effects::{EffectRecipe, EffectStep};
    let recipe = |id: &str, step| EffectRecipe {
        version: 1,
        id: id.into(),
        name: id.into(),
        steps: vec![step],
    };
    e.batch(
        &[
            Edit::SetEffects {
                clip_id: "v".into(),
                effects: vec![recipe("mono", EffectStep::Saturation { gain_milli: 0 })],
            },
            Edit::SetEffects {
                clip_id: "a".into(),
                effects: vec![recipe("muffled", EffectStep::LowPass { frequency_hz: 100 })],
            },
            Edit::SetEffects {
                clip_id: "a2".into(),
                effects: vec![recipe("muffled", EffectStep::LowPass { frequency_hz: 100 })],
            },
        ],
        e.revision,
    )
    .unwrap();
    let effected = dir.join("effected.mp4");
    render(
        &e.project,
        &effected,
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .unwrap();
    assert!(rms(&pcm(&effected), 1.0, 1.1) < peak * 0.1);
    let frame = Command::new(binary("ffmpeg"))
        .args(["-v", "error", "-ss", "0.5", "-i"])
        .arg(&effected)
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
    assert!(frame.status.success());
    let pixel = &frame.stdout[..3];
    assert!(
        pixel.iter().max().unwrap() - pixel.iter().min().unwrap() < 10,
        "not grayscale: {pixel:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn audio_only_echo_has_delayed_copy_and_keeps_clip_duration() {
    use blyacut_core::effects::{EffectRecipe, EffectStep};
    let dir = std::env::temp_dir().join(format!("blyacut-echo-{}", id()));
    std::fs::create_dir(&dir).unwrap();
    let input = dir.join("impulse.wav");
    fixture(
        &[
            "-f",
            "lavfi",
            "-i",
            "aevalsrc='if(lt(t,0.05),0.5*sin(2*PI*440*t),0)':s=48000:d=1",
            "-c:a",
            "pcm_s16le",
        ],
        &input,
    );
    let mut e = Editor::default();
    e.project.settings.width = 320;
    e.project.settings.height = 180;
    let asset = probe(&input).unwrap();
    let aid = asset.id.clone();
    e.import(asset).unwrap();
    e.batch(
        &[
            Edit::Append {
                asset_id: aid,
                clip_id: "impulse".into(),
                track_id: e.project.tracks[1].id.clone(),
            },
            Edit::SetEffects {
                clip_id: "impulse".into(),
                effects: vec![EffectRecipe {
                    version: 1,
                    id: "echo".into(),
                    name: "Short echo".into(),
                    steps: vec![EffectStep::Echo {
                        delay_ms: 200,
                        decay_milli: 500,
                    }],
                }],
            },
        ],
        e.revision,
    )
    .unwrap();
    let out = dir.join("echo.mp4");
    render(&e.project, &out, Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    let samples = pcm(&out);
    let original = rms(&samples, 0.01, 0.04);
    let echo = rms(&samples, 0.21, 0.24);
    assert!(original > 0.2);
    assert!((echo / original - 0.5).abs() < 0.1);
    assert!(rms(&samples, 0.1, 0.15) < 0.003);
    assert!(probe(&out).unwrap().duration_ms.abs_diff(1000) < 100);
    std::fs::remove_dir_all(dir).unwrap();
}
