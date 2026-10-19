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
/// RMS of the band around `hz` between `from` and `to` seconds.
fn band_rms(path: &std::path::Path, hz: u32, from: f64, to: f64) -> f64 {
    let out = Command::new(binary("ffmpeg"))
        .args(["-v", "error", "-i"])
        .arg(path)
        .args([
            "-vn",
            "-af",
            &format!("bandpass=f={hz}:width_type=h:w=60,bandpass=f={hz}:width_type=h:w=60"),
            "-f",
            "f32le",
            "-ac",
            "1",
            "-ar",
            "48000",
            "pipe:1",
        ])
        .output()
        .unwrap();
    let samples: Vec<f32> = out
        .stdout
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    let part = &samples[(from * 48000.0) as usize..(to * 48000.0) as usize];
    (part.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / part.len() as f64).sqrt()
}

#[test]
fn music_ducks_under_voice_only_while_voice_plays() {
    let dir = std::env::temp_dir().join(format!("blyacut-duck-{}", id()));
    std::fs::create_dir(&dir).unwrap();
    let talk = dir.join("talk.mp4");
    let music = dir.join("music.wav");
    // Voice: 1 kHz only between 1 s and 2 s, on the main video's own sound.
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=gray:s=160x90:r=30:d=3",
        "-f",
        "lavfi",
        "-i",
        "aevalsrc='0.3*sin(2*PI*1000*t)*between(t,1,2)':s=48000:d=3",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-shortest",
        talk.to_str().unwrap(),
    ]);
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "aevalsrc='0.25*sin(2*PI*220*t)':s=48000:d=3",
        music.to_str().unwrap(),
    ]);
    let mut e = Editor::default();
    e.project.settings.width = 160;
    e.project.settings.height = 90;
    let (v, m) = (probe(&talk).unwrap(), probe(&music).unwrap());
    let (vi, mi) = (v.id.clone(), m.id.clone());
    e.import(v).unwrap();
    e.import(m).unwrap();
    let (vt, at) = (
        e.project.tracks[0].id.clone(),
        e.project.tracks[1].id.clone(),
    );
    e.batch(
        &[
            Edit::Append {
                asset_id: vi,
                clip_id: "talk".into(),
                track_id: vt.clone(),
            },
            Edit::Append {
                asset_id: mi,
                clip_id: "music".into(),
                track_id: at,
            },
        ],
        e.revision,
    )
    .unwrap();
    assert!(
        e.execute(Edit::SetDucking {
            clip_id: "talk".into(),
            duck_db: 12
        })
        .is_err(),
        "video clips do not duck"
    );
    assert!(e
        .execute(Edit::SetDucking {
            clip_id: "music".into(),
            duck_db: 50
        })
        .is_err());

    let flat = dir.join("flat.mp4");
    render(&e.project, &flat, Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    let flat_ratio = band_rms(&flat, 220, 1.3, 1.8) / band_rms(&flat, 220, 0.2, 0.7);
    assert!(
        (flat_ratio - 1.0).abs() < 0.15,
        "no ducking without the setting: {flat_ratio}"
    );

    e.execute(Edit::SetDucking {
        clip_id: "music".into(),
        duck_db: 15,
    })
    .unwrap();
    let ducked = dir.join("ducked.mp4");
    render(
        &e.project,
        &ducked,
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .unwrap();
    let before = band_rms(&ducked, 220, 0.2, 0.7);
    let during = band_rms(&ducked, 220, 1.3, 1.8);
    let after = band_rms(&ducked, 220, 2.7, 2.95);
    eprintln!(
        "ducking: {:.1} dB under voice, recovered to {:.1} dB",
        20.0 * (during / before).log10(),
        20.0 * (after / before).log10()
    );
    let depth = 20.0 * (during / before).log10();
    assert!(
        (-17.0..=-12.0).contains(&depth),
        "about 15 dB under voice: {depth:.1} dB"
    );
    assert!(
        after > before * 0.8,
        "music recovers after voice: {after} vs {before}"
    );
    // Voice itself is untouched.
    assert!(band_rms(&ducked, 1000, 1.3, 1.8) > band_rms(&flat, 1000, 1.3, 1.8) * 0.9);
    std::fs::remove_dir_all(dir).unwrap();
}
