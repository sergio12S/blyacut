//! Agent-facing helpers: single-frame preview, loudness measurement and title cards.
use crate::{binary, normalize_filter, sec, video_effect_filters};
use blyacut_core::{Project, Result, TrackKind};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn ffmpeg(args: &[String]) -> Result<std::process::Output> {
    let out = Command::new(binary("ffmpeg"))
        .args(args)
        .output()
        .map_err(|e| format!("FFmpeg unavailable. Install FFmpeg or set BLYACUT_FFMPEG: {e}"))?;
    if out.status.success() {
        Ok(out)
    } else {
        Err(format!(
            "FFmpeg failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

fn new_file(out: &Path, ext: &str) -> Result<()> {
    if out.extension().and_then(|s| s.to_str()) != Some(ext) {
        return Err(format!("Output must end in .{ext}"));
    }
    if out.exists() {
        return Err("Output already exists; choose a new filename".into());
    }
    Ok(())
}

/// Render the exported look of one timeline instant to a new PNG, effects included.
pub fn preview_frame(project: &Project, time_ms: u64, out: &Path, width: u32) -> Result<()> {
    project.validate()?;
    new_file(out, "png")?;
    if !(64..=3840).contains(&width) {
        return Err("Preview width must be 64..=3840".into());
    }
    let s = &project.settings;
    let height = ((width as u64 * s.height as u64 / s.width as u64) / 2 * 2).max(2);
    let clip = project
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Video && !t.clips.is_empty())
        .and_then(|t| {
            t.clips
                .iter()
                .find(|c| c.position_ms <= time_ms && time_ms < c.position_ms + c.duration())
        });
    let base = [
        "-nostdin",
        "-v",
        "error",
        "-n",
        "-protocol_whitelist",
        "file,pipe",
    ];
    let mut args: Vec<String> = base.iter().map(|s| s.to_string()).collect();
    match clip {
        None => args.extend([
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            format!("color=c=black:s={width}x{height}"),
        ]),
        Some(c) => {
            let asset = project
                .assets
                .iter()
                .find(|a| a.id == c.asset_id)
                .ok_or("Unknown asset")?;
            if !asset.path.is_file() {
                return Err(format!("Missing media: {}", asset.path.display()));
            }
            let mut chain = normalize_filter(project);
            let effects = video_effect_filters(c);
            if !effects.is_empty() {
                chain = format!("{chain},{effects},format=yuv420p");
            }
            // Decode from the clip start so fades and trails see the same history as export.
            args.extend([
                "-ss".into(),
                sec(c.source_start_ms),
                "-i".into(),
                asset.path.to_string_lossy().into(),
                "-vf".into(),
                format!("{chain},scale={width}:{height}"),
                "-ss".into(),
                sec(time_ms - c.position_ms),
            ]);
        }
    }
    args.extend(["-frames:v".into(), "1".into(), out.to_string_lossy().into()]);
    ffmpeg(&args).map(|_| ())
}

#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct Loudness {
    pub mean_db: f64,
    pub peak_db: f64,
}

/// Mean and peak level of a file's first audio stream, in dBFS.
pub fn loudness(path: &Path) -> Result<Loudness> {
    let out = ffmpeg(&[
        "-nostdin".into(),
        "-hide_banner".into(),
        "-protocol_whitelist".into(),
        "file,pipe".into(),
        "-i".into(),
        path.to_string_lossy().into(),
        "-map".into(),
        "0:a:0".into(),
        "-af".into(),
        "volumedetect".into(),
        "-f".into(),
        "null".into(),
        "-".into(),
    ])?;
    let log = String::from_utf8_lossy(&out.stderr);
    let value = |key: &str| {
        log.lines()
            .find_map(|l| l.split(key).nth(1))
            .and_then(|v| v.trim().trim_end_matches("dB").trim().parse::<f64>().ok())
            .ok_or_else(|| format!("Could not measure {key}"))
    };
    Ok(Loudness {
        mean_db: value("mean_volume:")?,
        peak_db: value("max_volume:")?,
    })
}

pub struct TitleStyle {
    pub text_rgb: [u8; 3],
    pub background_rgb: [u8; 3],
}

/// System fonts with Latin and Cyrillic coverage; `BLYACUT_FONT` overrides.
pub fn font_path() -> Result<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::env::var_os("BLYACUT_FONT")
        .map(|p| vec![p.into()])
        .unwrap_or_default();
    candidates.extend(
        [
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/Library/Fonts/Arial Unicode.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/noto/NotoSans-Bold.ttf",
        ]
        .map(PathBuf::from),
    );
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| "No usable TrueType font found; set BLYACUT_FONT".into())
}

/// Rasterize centered text (up to 4 lines) into an RGB frame of the project size.
pub fn title_frame(text: &str, width: u32, height: u32, style: &TitleStyle) -> Result<Vec<u8>> {
    use ab_glyph::{point, Font, FontVec, PxScale, ScaleFont};
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    if text.trim().is_empty() || lines.len() > 4 || text.chars().count() > 200 {
        return Err("Title text must be 1–200 characters on at most 4 lines".into());
    }
    let font = FontVec::try_from_vec(std::fs::read(font_path()?).map_err(|e| e.to_string())?)
        .map_err(|_| "Unreadable font")?;
    let measure = |px: f32, line: &str| {
        let f = font.as_scaled(PxScale::from(px));
        line.chars()
            .map(|c| f.h_advance(f.glyph_id(c)))
            .sum::<f32>()
    };
    // Largest size whose widest line fits 84% of the frame and whose block fits 70% of its height.
    let mut px = height as f32 * 0.7 / lines.len() as f32 / 1.25;
    while px > 8.0 && lines.iter().any(|l| measure(px, l) > width as f32 * 0.84) {
        px *= 0.94;
    }
    let scaled = font.as_scaled(PxScale::from(px));
    let line_h = px * 1.25;
    let block = line_h * lines.len() as f32;
    let mut frame = style.background_rgb.repeat((width * height) as usize);
    for (i, line) in lines.iter().enumerate() {
        let mut x = (width as f32 - measure(px, line)) / 2.0;
        let baseline = (height as f32 - block) / 2.0 + line_h * i as f32 + scaled.ascent();
        for c in line.chars() {
            let id = scaled.glyph_id(c);
            let glyph = id.with_scale_and_position(PxScale::from(px), point(x, baseline));
            x += scaled.h_advance(id);
            if let Some(outline) = font.outline_glyph(glyph) {
                let b = outline.px_bounds();
                outline.draw(|gx, gy, coverage| {
                    let (px, py) = (b.min.x as i64 + gx as i64, b.min.y as i64 + gy as i64);
                    if px < 0 || py < 0 || px >= width as i64 || py >= height as i64 {
                        return;
                    }
                    let at = ((py as u32 * width + px as u32) * 3) as usize;
                    for ch in 0..3 {
                        let bg = frame[at + ch] as f32;
                        let fg = style.text_rgb[ch] as f32;
                        frame[at + ch] = (bg + (fg - bg) * coverage.min(1.0)).round() as u8;
                    }
                });
            }
        }
    }
    Ok(frame)
}

/// Encode a still title as a new H.264 MP4 (silent stereo track) in the project format.
pub fn title_card(
    project: &Project,
    text: &str,
    duration_ms: u64,
    style: &TitleStyle,
    out: &Path,
) -> Result<()> {
    new_file(out, "mp4")?;
    if !(500..=60000).contains(&duration_ms) {
        return Err("Title duration must be 500..=60000 ms".into());
    }
    let s = &project.settings;
    let frame = title_frame(text, s.width, s.height, style)?;
    let parent = out.parent().ok_or("Output needs a directory")?;
    let raw = parent.join(format!(".blyacut-title-{}.rgb", blyacut_core::id()));
    std::fs::write(&raw, frame).map_err(|e| e.to_string())?;
    let result = ffmpeg(&[
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
        "-n".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgb24".into(),
        "-s".into(),
        format!("{}x{}", s.width, s.height),
        "-framerate".into(),
        format!("{}/{}", s.fps_num, s.fps_den),
        "-stream_loop".into(),
        "-1".into(),
        "-i".into(),
        raw.to_string_lossy().into(),
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        "anullsrc=r=48000:cl=stereo".into(),
        "-t".into(),
        sec(duration_ms),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "veryfast".into(),
        "-tune".into(),
        "stillimage".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        out.to_string_lossy().into(),
    ]);
    let _ = std::fs::remove_file(&raw);
    result.map(|_| ())
}

pub fn parse_hex(color: &str) -> Result<[u8; 3]> {
    let hex = color.strip_prefix('#').unwrap_or(color);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Colors are #RRGGBB".into());
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string());
    Ok([byte(0)?, byte(2)?, byte(4)?])
}
