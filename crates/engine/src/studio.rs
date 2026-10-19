//! Agent-facing helpers: single-frame preview, loudness measurement and title cards.
use crate::{binary, normalize_filter, sec, video_effect_filters};
use blyacut_core::{Project, Result};
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

/// Render the exported look of one timeline instant to a new PNG: main track, effects and overlays.
pub fn preview_frame(project: &Project, time_ms: u64, out: &Path, width: u32) -> Result<()> {
    project.validate()?;
    new_file(out, "png")?;
    if !(64..=3840).contains(&width) {
        return Err("Preview width must be 64..=3840".into());
    }
    let active: Vec<_> = project
        .tracks
        .iter()
        .filter(|t| project.is_overlay_track(t))
        .flat_map(|t| &t.clips)
        .filter(|c| c.position_ms <= time_ms && time_ms < c.position_ms + c.duration())
        .collect();
    if active.is_empty() {
        return main_frame(project, time_ms, out, width);
    }
    let parent = out.parent().ok_or("Output needs a directory")?;
    let base = parent.join(format!(".blyacut-frame-{}.png", blyacut_core::id()));
    let result = (|| {
        main_frame(project, time_ms, &base, project.settings.width)?;
        let mut args: Vec<String> = [
            "-nostdin",
            "-v",
            "error",
            "-n",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        args.push(base.to_string_lossy().into());
        let mut graph = vec![];
        let mut current = "0:v".to_string();
        for (i, clip) in active.iter().enumerate() {
            let asset = project
                .assets
                .iter()
                .find(|a| a.id == clip.asset_id)
                .ok_or("Unknown asset")?;
            if !asset.still {
                args.extend([
                    "-ss".into(),
                    sec(clip.source_start_ms + time_ms - clip.position_ms),
                ]);
            }
            args.extend(["-i".into(), asset.path.to_string_lossy().into()]);
            let input = i + 1;
            graph.push(format!(
                "[{input}:v]{}[o{input}]",
                crate::overlay_layer(project, clip)
            ));
            graph.push(format!(
                "[{current}][o{input}]overlay={}[v{input}]",
                crate::overlay_position(clip)
            ));
            current = format!("v{input}");
        }
        let s = &project.settings;
        let height = ((width as u64 * s.height as u64 / s.width as u64) / 2 * 2).max(2);
        graph.push(format!("[{current}]scale={width}:{height}[out]"));
        args.extend([
            "-filter_complex".into(),
            graph.join(";"),
            "-map".into(),
            "[out]".into(),
            "-frames:v".into(),
            "1".into(),
            out.to_string_lossy().into(),
        ]);
        ffmpeg(&args).map(|_| ())
    })();
    let _ = std::fs::remove_file(&base);
    result
}

/// The main video track alone at one instant.
fn main_frame(project: &Project, time_ms: u64, out: &Path, width: u32) -> Result<()> {
    let s = &project.settings;
    let height = ((width as u64 * s.height as u64 / s.width as u64) / 2 * 2).max(2);
    let clip = project.main_video_track().and_then(|t| {
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
            args.extend(crate::media_input(project, asset, c.source_start_ms));
            args.extend([
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

/// Where and how large a text block is drawn.
pub struct TextLayout {
    /// Vertical centre of the block as a fraction of the frame height.
    pub center_y: f32,
    /// Maximum block height as a fraction of the frame height.
    pub block: f32,
    pub backing: Backing,
}
/// What keeps text legible over busy video.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Backing {
    None,
    /// Soft offset shadow.
    Shadow,
    /// Semi-transparent dark plate behind the block.
    Box,
    /// Dark stroke around every glyph.
    Outline,
}
impl std::str::FromStr for Backing {
    type Err = String;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "none" => Ok(Self::None),
            "shadow" => Ok(Self::Shadow),
            "box" => Ok(Self::Box),
            "outline" => Ok(Self::Outline),
            _ => Err("style must be shadow, box, outline or none".into()),
        }
    }
}

/// Rasterize centered text (up to 4 lines) into an RGBA frame; `None` background is transparent.
pub fn render_text(
    text: &str,
    width: u32,
    height: u32,
    text_rgb: [u8; 3],
    background: Option<[u8; 3]>,
    layout: &TextLayout,
) -> Result<Vec<u8>> {
    use ab_glyph::{point, Font, FontVec, PxScale, ScaleFont};
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    if text.trim().is_empty() || lines.len() > 4 || text.chars().count() > 200 {
        return Err("Text must be 1–200 characters on at most 4 lines".into());
    }
    let font = FontVec::try_from_vec(std::fs::read(font_path()?).map_err(|e| e.to_string())?)
        .map_err(|_| "Unreadable font")?;
    let measure = |px: f32, line: &str| {
        let f = font.as_scaled(PxScale::from(px));
        line.chars()
            .map(|c| f.h_advance(f.glyph_id(c)))
            .sum::<f32>()
    };
    // Largest size whose widest line fits 84% of the frame and whose block fits `layout.block`.
    let mut px = height as f32 * layout.block / lines.len() as f32 / 1.25;
    while px > 8.0 && lines.iter().any(|l| measure(px, l) > width as f32 * 0.84) {
        px *= 0.94;
    }
    let scaled = font.as_scaled(PxScale::from(px));
    let line_h = px * 1.25;
    let block = line_h * lines.len() as f32;
    let top = (height as f32 * layout.center_y - block / 2.0).clamp(0.0, height as f32 - block);
    let mut frame = match background {
        Some([r, g, b]) => [r, g, b, 255].repeat((width * height) as usize),
        None => vec![0; (width * height * 4) as usize],
    };
    // Porter-Duff "over" of one coloured pixel with coverage alpha.
    let mut paint = |x: i64, y: i64, rgb: [u8; 3], alpha: f32| {
        if x < 0 || y < 0 || x >= width as i64 || y >= height as i64 || alpha <= 0.0 {
            return;
        }
        let at = ((y as u32 * width + x as u32) * 4) as usize;
        let dst_a = frame[at + 3] as f32 / 255.0;
        let out_a = alpha + dst_a * (1.0 - alpha);
        for ch in 0..3 {
            let dst = frame[at + ch] as f32;
            frame[at + ch] =
                ((rgb[ch] as f32 * alpha + dst * dst_a * (1.0 - alpha)) / out_a).round() as u8;
        }
        frame[at + 3] = (out_a * 255.0).round() as u8;
    };
    if layout.backing == Backing::Box {
        let widest = lines.iter().map(|l| measure(px, l)).fold(0.0, f32::max);
        let pad = px * 0.45;
        let (x0, x1) = (
            (width as f32 - widest) / 2.0 - pad,
            (width as f32 + widest) / 2.0 + pad,
        );
        let (y0, y1) = (top - pad * 0.5, top + block + pad * 0.3);
        for y in y0.max(0.0) as i64..y1.min(height as f32) as i64 {
            for x in x0.max(0.0) as i64..x1.min(width as f32) as i64 {
                paint(x, y, [0, 0, 0], 0.6);
            }
        }
    }
    let d = (px * 0.05).max(1.0);
    let mut passes: Vec<(f32, f32, [u8; 3], f32)> = match layout.backing {
        Backing::Shadow => vec![(d, d, [0, 0, 0], 0.65)],
        Backing::Outline => (0..12)
            .map(|k| {
                let a = k as f32 * std::f32::consts::TAU / 12.0;
                (a.cos() * d * 1.4, a.sin() * d * 1.4, [0, 0, 0], 1.0)
            })
            .collect(),
        Backing::None | Backing::Box => vec![],
    };
    passes.push((0.0, 0.0, text_rgb, 1.0));
    for &(dx, dy, rgb, strength) in &passes {
        for (i, line) in lines.iter().enumerate() {
            let mut x = (width as f32 - measure(px, line)) / 2.0 + dx;
            let baseline = top + line_h * i as f32 + scaled.ascent() + dy;
            for c in line.chars() {
                let id = scaled.glyph_id(c);
                let glyph = id.with_scale_and_position(PxScale::from(px), point(x, baseline));
                x += scaled.h_advance(id);
                if let Some(outline) = font.outline_glyph(glyph) {
                    let b = outline.px_bounds();
                    outline.draw(|gx, gy, coverage| {
                        paint(
                            b.min.x as i64 + gx as i64,
                            b.min.y as i64 + gy as i64,
                            rgb,
                            coverage.min(1.0) * strength,
                        )
                    });
                }
            }
        }
    }
    Ok(frame)
}

/// Opaque full-frame title (RGB), as used by title cards.
pub fn title_frame(text: &str, width: u32, height: u32, style: &TitleStyle) -> Result<Vec<u8>> {
    let layout = TextLayout {
        center_y: 0.5,
        block: 0.7,
        backing: Backing::None,
    };
    let rgba = render_text(
        text,
        width,
        height,
        style.text_rgb,
        Some(style.background_rgb),
        &layout,
    )?;
    Ok(rgba
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect())
}

/// Transparent full-frame PNG with text; place it on an overlay track over video.
pub fn text_overlay(
    project: &Project,
    text: &str,
    text_rgb: [u8; 3],
    center_y: f32,
    backing: Backing,
    out: &Path,
) -> Result<()> {
    new_file(out, "png")?;
    if !(0.05..=0.95).contains(&center_y) {
        return Err("Text position must be 0.05..=0.95 of the frame height".into());
    }
    let s = &project.settings;
    let layout = TextLayout {
        center_y,
        block: 0.22,
        backing,
    };
    let frame = render_text(text, s.width, s.height, text_rgb, None, &layout)?;
    let parent = out.parent().ok_or("Output needs a directory")?;
    let raw = parent.join(format!(".blyacut-text-{}.rgba", blyacut_core::id()));
    std::fs::write(&raw, frame).map_err(|e| e.to_string())?;
    let result = ffmpeg(&[
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
        "-n".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgba".into(),
        "-s".into(),
        format!("{}x{}", s.width, s.height),
        "-i".into(),
        raw.to_string_lossy().into(),
        "-frames:v".into(),
        "1".into(),
        out.to_string_lossy().into(),
    ]);
    let _ = std::fs::remove_file(&raw);
    result.map(|_| ())
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
