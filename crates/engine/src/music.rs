//! Agent-facing music analysis, previews and thumbnail variants.
use crate::{binary, probe, studio, visualizer};
use blyacut_core::{Project, Result};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Serialize)]
pub struct Candidate {
    pub kind: &'static str,
    pub start_ms: u64,
    pub duration_ms: u64,
    pub energy: f64,
    pub onset_count: usize,
}
fn ffmpeg(args: &[String]) -> Result<std::process::Output> {
    let out = Command::new(binary("ffmpeg"))
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(out)
    } else {
        Err(format!("FFmpeg: {}", String::from_utf8_lossy(&out.stderr)))
    }
}
fn safe_prefix(prefix: &str) -> bool {
    !prefix.is_empty()
        && prefix.len() <= 48
        && prefix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Bass-energy heuristics, not semantic chorus/beat detection.
pub fn candidates(input: &Path, seconds: u32) -> Result<Vec<Candidate>> {
    if !(5..=30).contains(&seconds) {
        return Err("seconds must be 5..=30".into());
    }
    let asset = probe(input)?;
    if !asset.has_audio || asset.duration_ms > 600_000 || asset.duration_ms < seconds as u64 * 3000
    {
        return Err("Input must have audio, last at most 10 minutes and fit three non-overlapping candidates".into());
    }
    let raw = ffmpeg(&[
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
        "-protocol_whitelist".into(),
        "file,pipe".into(),
        "-i".into(),
        asset.path.to_string_lossy().into(),
        "-map".into(),
        "0:a:0".into(),
        "-vn".into(),
        "-af".into(),
        "lowpass=f=150".into(),
        "-ac".into(),
        "1".into(),
        "-ar".into(),
        "2000".into(),
        "-f".into(),
        "f32le".into(),
        "pipe:1".into(),
    ])?;
    let rms: Vec<f64> = raw
        .stdout
        .chunks(2000)
        .map(|w| {
            let mut sum = 0.;
            let mut n = 0;
            for c in w.as_chunks::<4>().0 {
                let v = f32::from_le_bytes(*c) as f64;
                if v.is_finite() {
                    sum += v * v;
                    n += 1;
                }
            }
            if n == 0 {
                0.
            } else {
                (sum / n as f64).sqrt()
            }
        })
        .collect();
    let window = seconds as usize * 4;
    if rms.len() < window * 3 {
        return Err("Decoded audio is too short".into());
    }
    let mut sorted = rms.clone();
    sorted.sort_by(f64::total_cmp);
    let peak = sorted[sorted.len() * 9 / 10].max(0.003);
    let mut onsets = vec![0_u8; rms.len()];
    for i in 4..rms.len() {
        let before = rms[i - 4..i].iter().sum::<f64>() / 4.;
        if rms[i] > peak * 0.35 && rms[i] > before * 1.35 {
            onsets[i] = 1;
        }
    }
    let total = rms.len().min((asset.duration_ms / 250) as usize);
    let mut result = vec![];
    for (part, kind) in ["bass_energy", "rising_energy", "rhythmic"]
        .into_iter()
        .enumerate()
    {
        let begin = part * total / 3;
        let end = (part + 1) * total / 3;
        let best = (begin..=end - window)
            .step_by(4)
            .map(|i| {
                let part = &rms[i..i + window];
                let mean = part.iter().sum::<f64>() / window as f64 / peak;
                let rise = (part[window / 2..].iter().sum::<f64>()
                    - part[..window / 2].iter().sum::<f64>())
                    / (window as f64 * peak);
                let hits = onsets[i..i + window]
                    .iter()
                    .map(|n| *n as usize)
                    .sum::<usize>();
                let score = match kind {
                    "rising_energy" => rise + mean * 0.15,
                    "rhythmic" => hits as f64 / window as f64 + mean * 0.1,
                    _ => mean,
                };
                (i, score, mean, hits)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .ok_or("Could not find three non-overlapping windows")?;
        result.push(Candidate {
            kind,
            start_ms: best.0 as u64 * 250,
            duration_ms: seconds as u64 * 1000,
            energy: best.2,
            onset_count: best.3,
        });
    }
    Ok(result)
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Three short visual comparisons, not timeline edits. Audio is re-encoded for accurate seeking.
pub fn previews(
    input: &Path,
    dir: &Path,
    prefix: &str,
    start_ms: u64,
    seconds: u32,
    intensity: u32,
) -> Result<Vec<PathBuf>> {
    if !safe_prefix(prefix)
        || !dir.is_dir()
        || !(3..=15).contains(&seconds)
        || !(1..=100).contains(&intensity)
    {
        return Err("Invalid prefix, output directory, preview duration or intensity".into());
    }
    let asset = probe(input)?;
    if asset.still
        || !asset.has_audio
        || start_ms
            .checked_add(seconds as u64 * 1000)
            .is_none_or(|x| x > asset.duration_ms)
    {
        return Err("Preview range must fit a video with audio".into());
    }
    let looks = ["dirty", "neon", "vhs"];
    let outputs: Vec<PathBuf> = looks
        .iter()
        .map(|look| dir.join(format!("{prefix}-{look}.mp4")))
        .collect();
    if outputs.iter().any(|p| p.exists()) {
        return Err("Preview output already exists; choose another prefix".into());
    }
    let scratch =
        Scratch(std::env::temp_dir().join(format!("blyacut-previews-{}", blyacut_core::id())));
    std::fs::create_dir(&scratch.0).map_err(|e| e.to_string())?;
    let clip = scratch.0.join("clip.mp4");
    ffmpeg(&[
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
        "-n".into(),
        "-protocol_whitelist".into(),
        "file,pipe".into(),
        "-ss".into(),
        format!("{:.3}", start_ms as f64 / 1000.),
        "-i".into(),
        asset.path.to_string_lossy().into(),
        "-t".into(),
        seconds.to_string(),
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "0:a:0".into(),
        "-vf".into(),
        "scale=w='min(720,iw)':h='min(720,ih)':force_original_aspect_ratio=decrease:force_divisible_by=2".into(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "ultrafast".into(),
        "-crf".into(),
        "18".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
        clip.to_string_lossy().into(),
    ])?;
    for (look, out) in looks.iter().zip(&outputs) {
        let options = visualizer::PhonkOptions {
            look: look.to_string(),
            drop: false,
            ..Default::default()
        };
        visualizer::render_phonk(&clip, out, intensity, &options)?;
    }
    Ok(outputs)
}

/// Three thumbnail frames with a readable supplied title; no generative model is involved.
pub fn thumbnails(
    input: &Path,
    dir: &Path,
    prefix: &str,
    title: &str,
    times: &[u64],
) -> Result<Vec<PathBuf>> {
    if !safe_prefix(prefix)
        || !dir.is_dir()
        || title.trim().is_empty()
        || title.chars().count() > 100
        || title.lines().count() > 2
        || times.len() != 3
    {
        return Err("Need an existing directory, safe prefix, title and three times".into());
    }
    let asset = probe(input)?;
    if asset.width < 64 || asset.height < 64 || times.iter().any(|t| *t >= asset.duration_ms) {
        return Err("Thumbnail times must be inside a video".into());
    }
    let outputs: Vec<PathBuf> = (1..=3)
        .map(|i| dir.join(format!("{prefix}-thumbnail-{i}.png")))
        .collect();
    if outputs.iter().any(|p| p.exists()) {
        return Err("Thumbnail output already exists".into());
    }
    let scratch =
        Scratch(std::env::temp_dir().join(format!("blyacut-thumbs-{}", blyacut_core::id())));
    std::fs::create_dir(&scratch.0).map_err(|e| e.to_string())?;
    let mut project = Project::default();
    project.settings.width = 1280;
    project.settings.height = 720;
    for (i, (at, out)) in times.iter().zip(&outputs).enumerate() {
        let overlay = scratch.0.join(format!("text-{i}.png"));
        studio::text_overlay(
            &project,
            title,
            if i == 1 {
                [255, 220, 80]
            } else {
                [255, 255, 255]
            },
            if i == 2 { 0.2 } else { 0.8 },
            studio::Backing::Box,
            &overlay,
        )?;
        ffmpeg(&["-nostdin".into(),"-v".into(),"error".into(),"-n".into(),"-protocol_whitelist".into(),"file,pipe".into(),"-ss".into(),format!("{:.3}",*at as f64/1000.),"-i".into(),asset.path.to_string_lossy().into(),"-i".into(),overlay.to_string_lossy().into(),"-filter_complex".into(),"[0:v]split=2[bg][fg];[bg]scale=1280:720:force_original_aspect_ratio=increase,crop=1280:720,boxblur=20:1,eq=brightness=-0.22[back];[fg]scale=1280:720:force_original_aspect_ratio=decrease[front];[back][front]overlay=(W-w)/2:(H-h)/2[base];[base][1:v]overlay=0:0[v]".into(),"-map".into(),"[v]".into(),"-frames:v".into(),"1".into(),out.to_string_lossy().into()])?;
    }
    Ok(outputs)
}
