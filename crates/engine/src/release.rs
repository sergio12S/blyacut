//! Create a landscape upload and three captioned vertical teasers from a finished video.
use crate::{binary, probe, studio};
use blyacut_core::{
    captions::{parse_srt, Caption},
    Project, Result,
};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

#[derive(Serialize)]
pub struct ReleasePackage {
    pub full_video: PathBuf,
    pub shorts: Vec<Short>,
    pub caption_source: String,
    pub warnings: Vec<String>,
}
#[derive(Serialize)]
pub struct Short {
    pub path: PathBuf,
    pub start_ms: u64,
    pub duration_ms: u64,
    pub captions: usize,
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[String]) -> Result<std::process::Output> {
    let out = Command::new(binary("ffmpeg"))
        .args(args)
        .output()
        .map_err(|e| format!("FFmpeg unavailable: {e}"))?;
    if out.status.success() {
        Ok(out)
    } else {
        Err(format!(
            "FFmpeg failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

pub(crate) fn run_video(args: &[String], cancel: &AtomicBool, scratch: &Path) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled before export".into());
    }
    let log_path = scratch.join(format!("ffmpeg-{}.log", blyacut_core::id()));
    let log = std::fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let mut child = Command::new(binary("ffmpeg"))
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|e| format!("FFmpeg unavailable: {e}"))?;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Cancelled; completed or partial output files may remain".into());
        }
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() => return Ok(()),
            Some(_) => {
                return Err(format!(
                    "FFmpeg failed: {}",
                    std::fs::read_to_string(log_path).unwrap_or_default()
                ))
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// Select one loud 15s-style window from each third. This estimates energy, not lyrical quality.
fn select_starts(path: &Path, duration_ms: u64, length_ms: u64) -> Result<Vec<u64>> {
    let out = run(&[
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
        "-protocol_whitelist".into(),
        "file,pipe".into(),
        "-i".into(),
        path.to_string_lossy().into(),
        "-map".into(),
        "0:a:0".into(),
        "-vn".into(),
        "-ac".into(),
        "1".into(),
        "-ar".into(),
        "2000".into(),
        "-f".into(),
        "f32le".into(),
        "pipe:1".into(),
    ])?;
    let energy: Vec<f64> = out
        .stdout
        .chunks(2000 * 4)
        .map(|window| {
            let (sum, n) = window
                .as_chunks::<4>()
                .0
                .iter()
                .fold((0.0, 0_u32), |(s, n), sample| {
                    let v = f32::from_le_bytes(*sample) as f64;
                    if v.is_finite() {
                        (s + v * v, n + 1)
                    } else {
                        (s, n)
                    }
                });
            if n == 0 {
                0.0
            } else {
                sum / n as f64
            }
        })
        .collect();
    let seconds = (length_ms / 1000) as usize;
    let total = (duration_ms / 1000) as usize;
    if total < seconds * 3 || energy.len() < seconds * 3 {
        return Err("Input needs at least three times the Short duration".into());
    }
    let mut starts = vec![];
    for part in 0..3 {
        let begin = part * total / 3;
        let end = if part == 2 {
            total
        } else {
            (part + 1) * total / 3
        };
        let last = end
            .saturating_sub(seconds)
            .min(energy.len().saturating_sub(seconds));
        let best = (begin..=last)
            .max_by(|a, b| {
                let score = |i: usize| energy[i..i + seconds].iter().sum::<f64>();
                score(*a).total_cmp(&score(*b))
            })
            .ok_or("Not enough audio for three Shorts")?;
        starts.push(best as u64 * 1000);
    }
    Ok(starts)
}

fn embedded_captions(path: &Path) -> Vec<Caption> {
    let result = Command::new(binary("ffmpeg"))
        .args([
            "-nostdin",
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ])
        .arg(path)
        .args(["-map", "0:s:0", "-f", "srt", "pipe:1"])
        .output();
    result
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| parse_srt(&s).ok())
        .unwrap_or_default()
}

/// Inputs must already be scoped by the gateway. Output names are fixed and never overwritten.
pub fn render(
    input: &Path,
    output_dir: &Path,
    prefix: &str,
    title: &str,
    short_seconds: u32,
    starts_ms: Option<Vec<u64>>,
) -> Result<ReleasePackage> {
    render_controlled(
        input,
        output_dir,
        prefix,
        title,
        short_seconds,
        starts_ms,
        None,
        "lines",
        &AtomicBool::new(false),
        |_| {},
    )
}

/// Uses supplied SRT in timeline coordinates; cancellation stops the active video encoder.
#[allow(clippy::too_many_arguments)]
pub fn render_controlled(
    input: &Path,
    output_dir: &Path,
    prefix: &str,
    title: &str,
    short_seconds: u32,
    starts_ms: Option<Vec<u64>>,
    srt: Option<&str>,
    caption_mode: &str,
    cancel: &AtomicBool,
    progress: impl Fn(u8),
) -> Result<ReleasePackage> {
    if !["lines", "word", "none"].contains(&caption_mode) {
        return Err("caption_mode must be lines, word or none".into());
    }
    if caption_mode == "word" && srt.is_none() {
        return Err("Word mode requires supplied word-timed SRT".into());
    }
    if caption_mode == "none" && srt.is_some() {
        return Err("caption_mode none cannot be combined with captions_srt".into());
    }
    if prefix.is_empty()
        || prefix.len() > 48
        || !prefix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(
            "prefix must contain only ASCII letters, numbers, '-' or '_' (1–48 characters)".into(),
        );
    }
    if title.trim().is_empty() || title.chars().count() > 100 || title.lines().count() > 2 {
        return Err("title must be 1–100 characters on at most 2 lines".into());
    }
    if !(5..=30).contains(&short_seconds) {
        return Err("short_seconds must be 5..=30".into());
    }
    if !output_dir.is_dir() {
        return Err("output_dir must be an existing directory".into());
    }
    let asset = probe(input)?;
    if asset.still
        || asset.width < 64
        || asset.height < 64
        || !asset.has_audio
        || asset.duration_ms > 600_000
    {
        return Err(
            "Input must be a video with audio, at least 64x64 and at most 10 minutes".into(),
        );
    }
    let duration_ms = short_seconds as u64 * 1000;
    let paths = [
        output_dir.join(format!("{prefix}-full.mp4")),
        output_dir.join(format!("{prefix}-short-1.mp4")),
        output_dir.join(format!("{prefix}-short-2.mp4")),
        output_dir.join(format!("{prefix}-short-3.mp4")),
    ];
    if paths.iter().any(|p| p.exists()) {
        return Err("One or more package output files already exist; choose another prefix".into());
    }
    let starts = match starts_ms {
        Some(v)
            if v.len() == 3
                && v.iter().all(|s| {
                    s.checked_add(duration_ms)
                        .is_some_and(|end| end <= asset.duration_ms)
                }) =>
        {
            v
        }
        Some(_) => return Err("starts_ms must contain exactly three in-range start times".into()),
        None => select_starts(&asset.path, asset.duration_ms, duration_ms)?,
    };
    let supplied = srt.is_some();
    let captions = if caption_mode == "none" {
        vec![]
    } else if let Some(srt) = srt {
        let mut captions = parse_srt(srt)?;
        blyacut_core::captions::validate(&mut captions)?;
        if captions.iter().any(|c| c.text.lines().count() > 2) {
            return Err("Supplied captions must have at most two lines per cue".into());
        }
        if caption_mode == "word"
            && captions
                .iter()
                .any(|c| c.text.split_whitespace().count() != 1)
        {
            return Err("Word mode needs exactly one timed word per SRT cue".into());
        }
        captions
    } else {
        embedded_captions(&asset.path)
    };
    if starts.iter().any(|start| {
        captions
            .iter()
            .filter(|c| c.start_ms < start + duration_ms && c.end_ms > *start)
            .count()
            > 40
    }) {
        return Err(
            "Too many captions in a Short (maximum 40); use shorter excerpts or fewer cues".into(),
        );
    }
    let scratch =
        Scratch(std::env::temp_dir().join(format!("blyacut-release-{}", blyacut_core::id())));
    std::fs::create_dir(&scratch.0).map_err(|e| e.to_string())?;
    let landscape = "[0:v]split=2[bg][fg];[bg]scale=1920:1080:force_original_aspect_ratio=increase,crop=1920:1080,boxblur=20:1,eq=brightness=-0.22:saturation=0.8[back];[fg]scale=1920:1080:force_original_aspect_ratio=decrease[front];[back][front]overlay=(W-w)/2:(H-h)/2,setsar=1[v]";
    progress(0);
    let mut full_args = vec![
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
        "-n".into(),
        "-protocol_whitelist".into(),
        "file,pipe".into(),
        "-i".into(),
        asset.path.to_string_lossy().into(),
        "-filter_complex".into(),
        landscape.into(),
        "-map".into(),
        "[v]".into(),
        "-map".into(),
        "0:a:0".into(),
        "-map".into(),
        "0:s?".into(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "fast".into(),
        "-crf".into(),
        "20".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "copy".into(),
        "-c:s".into(),
        "copy".into(),
        "-map_metadata".into(),
        "0".into(),
        "-movflags".into(),
        "+faststart".into(),
        paths[0].to_string_lossy().into(),
    ];
    if caption_mode == "none" {
        if let Some(i) = full_args
            .windows(2)
            .position(|w| w[0] == "-map" && w[1] == "0:s?")
        {
            full_args.drain(i..i + 2);
        }
        if let Some(i) = full_args
            .windows(2)
            .position(|w| w[0] == "-c:s" && w[1] == "copy")
        {
            full_args.drain(i..i + 2);
        }
    }
    run_video(&full_args, cancel, &scratch.0)?;
    progress(1);
    let mut shorts = Vec::new();
    for (i, start) in starts.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled; completed or partial output files may remain".into());
        }
        let relevant: Vec<_> = captions
            .iter()
            .filter(|c| c.start_ms < start + duration_ms && c.end_ms > *start)
            .collect();
        let mut pieces: Vec<(String, u64, u64)> = relevant
            .iter()
            .filter_map(|c| {
                let text = c.text.replace('♪', "").trim().to_string();
                if text.is_empty() || text.chars().count() > 200 || text.lines().count() > 4 {
                    None
                } else {
                    Some((
                        text,
                        c.start_ms.saturating_sub(*start),
                        (c.end_ms - *start).min(duration_ms),
                    ))
                }
            })
            .collect();
        if pieces.is_empty() && caption_mode == "lines" {
            pieces.push((title.to_string(), 0, duration_ms));
        }
        if pieces.len() > 40 {
            return Err("Too many captions in a Short (maximum 40)".into());
        }
        let mut project = Project::default();
        project.settings.width = 720;
        project.settings.height = 1280;
        let mut args: Vec<String> = [
            "-nostdin",
            "-v",
            "error",
            "-n",
            "-protocol_whitelist",
            "file,pipe",
            "-ss",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        args.extend([
            format!("{:.3}", *start as f64 / 1000.0),
            "-i".into(),
            asset.path.to_string_lossy().into(),
        ]);
        for (j, (text, _, _)) in pieces.iter().enumerate() {
            let png = scratch.0.join(format!("text-{i}-{j}.png"));
            studio::text_overlay(
                &project,
                text,
                if caption_mode == "word" {
                    [160, 255, 70]
                } else {
                    [255, 255, 255]
                },
                0.70,
                studio::Backing::Box,
                &png,
            )?;
            args.extend([
                "-loop".into(),
                "1".into(),
                "-framerate".into(),
                "30".into(),
                "-i".into(),
                png.to_string_lossy().into(),
            ]);
        }
        let mut graph="[0:v]setpts=PTS-STARTPTS,split=2[bg][fg];[bg]scale=720:1280:force_original_aspect_ratio=increase,crop=720:1280,boxblur=20:1,eq=brightness=-0.22:saturation=0.8[back];[fg]scale=720:1280:force_original_aspect_ratio=decrease[front];[back][front]overlay=(W-w)/2:(H-h)/2[base0]".to_string();
        for (j, (_, begin, end)) in pieces.iter().enumerate() {
            graph.push_str(&format!(
                ";[base{j}][{}:v]overlay=0:0:enable='gte(t,{:.3})*lt(t,{:.3})'[base{}]",
                j + 1,
                *begin as f64 / 1000.0,
                *end as f64 / 1000.0,
                j + 1
            ));
        }
        graph.push_str(&format!(";[base{}]setsar=1[v]", pieces.len()));
        args.extend([
            "-filter_complex".into(),
            graph,
            "-map".into(),
            "[v]".into(),
            "-map".into(),
            "0:a:0".into(),
            "-t".into(),
            format!("{short_seconds}"),
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            "fast".into(),
            "-crf".into(),
            "20".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "192k".into(),
            "-movflags".into(),
            "+faststart".into(),
            paths[i + 1].to_string_lossy().into(),
        ]);
        run_video(&args, cancel, &scratch.0)?;
        shorts.push(Short {
            path: paths[i + 1].clone(),
            start_ms: *start,
            duration_ms,
            captions: relevant.len(),
        });
        progress((i + 2) as u8);
    }
    let mut warnings=vec!["Automatically selected sections are energy estimates; review musical and lyrical quality before publishing.".into()];
    if caption_mode == "none" {
        warnings.push("Subtitle tracks and overlays were omitted; text already burned into input pixels cannot be removed.".into());
    } else if !captions.is_empty() && !supplied {
        warnings.push("Embedded captions were rendered without verifying lyric accuracy or timing; review before publishing.".into());
    } else if captions.is_empty() {
        warnings.push(
            "No embedded subtitles found; Shorts show the supplied title instead of lyrics.".into(),
        );
    }
    Ok(ReleasePackage {
        full_video: paths[0].clone(),
        shorts,
        caption_source: if caption_mode == "none" {
            "none"
        } else if supplied {
            "provided SRT"
        } else if captions.is_empty() {
            "title"
        } else {
            "embedded subtitles"
        }
        .into(),
        warnings,
    })
}
