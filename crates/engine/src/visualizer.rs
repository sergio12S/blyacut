//! Offline bass-driven animation for an existing video. All input remains untouched.
use crate::{binary, probe};
use blyacut_core::Result;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Write as _,
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PhonkOptions {
    pub look: String,
    pub flash_color: String,
    pub flash: bool,
    pub glitch: bool,
    pub grain: bool,
    pub waveform: bool,
    pub drop: bool,
    pub drop_ms: Option<u64>,
}
impl Default for PhonkOptions {
    fn default() -> Self {
        Self {
            look: "dirty".into(),
            flash_color: "green".into(),
            flash: true,
            glitch: true,
            grain: true,
            waveform: true,
            drop: true,
            drop_ms: None,
        }
    }
}

#[derive(Serialize)]
pub struct VisualizerResult {
    pub analysis_windows: usize,
    pub detected_accents: usize,
    pub drop_ms: Option<u64>,
    pub drop_estimated: bool,
}

/// Render a new MP4, preserving the original audio stream without recompression.
/// The animation is driven by 100 ms windows of low-frequency RMS energy.
pub fn render(input: &Path, output: &Path, intensity: u32) -> Result<usize> {
    render_internal(input, output, intensity, None, &AtomicBool::new(false))
        .map(|x| x.analysis_windows)
}

pub fn render_phonk(
    input: &Path,
    output: &Path,
    intensity: u32,
    options: &PhonkOptions,
) -> Result<VisualizerResult> {
    render_internal(
        input,
        output,
        intensity,
        Some(options),
        &AtomicBool::new(false),
    )
}

pub fn render_phonk_controlled(
    input: &Path,
    output: &Path,
    intensity: u32,
    options: &PhonkOptions,
    cancel: &AtomicBool,
) -> Result<VisualizerResult> {
    render_internal(input, output, intensity, Some(options), cancel)
}

fn render_internal(
    input: &Path,
    output: &Path,
    intensity: u32,
    options: Option<&PhonkOptions>,
    cancel: &AtomicBool,
) -> Result<VisualizerResult> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled before visualizer export".into());
    }
    if !(1..=100).contains(&intensity) {
        return Err("intensity must be between 1 and 100".into());
    }
    if output.exists() || output.extension().and_then(|s| s.to_str()) != Some("mp4") {
        return Err("Output must be a new .mp4 file".into());
    }
    let asset = probe(input)?;
    if asset.still || asset.width < 64 || asset.height < 64 || !asset.has_audio {
        return Err("Input must be a video with sound and dimensions of at least 64 pixels".into());
    }
    if asset.duration_ms > 600_000 {
        return Err("Visualizer supports videos up to 10 minutes".into());
    }
    if let Some(o) = options {
        if !["dirty", "neon", "vhs", "warehouse"].contains(&o.look.as_str()) {
            return Err("look must be dirty, neon, vhs or warehouse".into());
        }
        if o.flash_color != "green" && o.flash_color != "red" {
            return Err("flash_color must be green or red".into());
        }
        if o.drop_ms.is_some_and(|at| at >= asset.duration_ms) {
            return Err("drop_ms must be inside the video".into());
        }
        if !o.drop && o.drop_ms.is_some() {
            return Err("drop_ms requires drop=true".into());
        }
    }
    let analysis = Command::new(binary("ffmpeg"))
        .args([
            "-nostdin",
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ])
        .arg(&asset.path)
        .args([
            "-map",
            "0:a:0",
            "-vn",
            "-af",
            "lowpass=f=150",
            "-ac",
            "1",
            "-ar",
            "8000",
            "-f",
            "f32le",
            "pipe:1",
        ])
        .output()
        .map_err(|e| format!("FFmpeg analysis: {e}"))?;
    if !analysis.status.success() {
        return Err(format!(
            "Audio analysis failed: {}",
            String::from_utf8_lossy(&analysis.stderr)
        ));
    }
    let energies: Vec<f64> = analysis
        .stdout
        .chunks(800 * 4)
        .map(|window| {
            let mut sum = 0.0;
            let mut n = 0;
            for sample in window.as_chunks::<4>().0 {
                let value = f32::from_le_bytes(*sample) as f64;
                if value.is_finite() {
                    sum += value * value;
                    n += 1;
                }
            }
            if n == 0 {
                0.0
            } else {
                (sum / n as f64).sqrt()
            }
        })
        .collect();
    if energies.is_empty() {
        return Err("Input audio contains no samples".into());
    }
    let mut sorted = energies.clone();
    sorted.sort_by(f64::total_cmp);
    let reference = sorted[(sorted.len() * 9 / 10).min(sorted.len() - 1)].max(0.003);
    // A drop is an estimate: find the largest rise in 1s bass energy after the intro.
    let estimated_drop =
        if options.is_some_and(|o| o.drop && o.drop_ms.is_none()) && energies.len() >= 80 {
            (50..energies.len().saturating_sub(10))
                .map(|i| {
                    let before = energies[i - 10..i].iter().sum::<f64>() / 10.0;
                    let after = energies[i..i + 10].iter().sum::<f64>() / 10.0;
                    (i, after - before)
                })
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .filter(|(_, change)| *change > reference * 0.12)
                .map(|(i, _)| i as u64 * 100)
        } else {
            None
        };
    let drop_ms = options.and_then(|o| {
        if o.drop {
            o.drop_ms.or(estimated_drop)
        } else {
            None
        }
    });
    let width = asset.width / 2 * 2;
    let height = asset.height / 2 * 2;
    let strength = intensity as f64 / 100.0;
    let mut commands = String::new();
    let mut envelope = 0.0_f64;
    let mut previous_energy = 0.0_f64;
    let mut last_accent: Option<usize> = None;
    let mut accents = 0;
    for (i, energy) in energies.iter().enumerate() {
        let target = (energy / reference).clamp(0.0, 1.0);
        // Quick attack, gentle release; silence naturally returns to the original framing.
        envelope += (target - envelope) * if target > envelope { 0.8 } else { 0.35 };
        let accent = options.is_some()
            && target > 0.55
            && *energy > previous_energy * 1.24
            && last_accent.is_none_or(|last| i - last >= 4);
        if accent {
            last_accent = Some(i);
            accents += 1;
        }
        let accent_envelope =
            last_accent.map_or(0.0, |last| (1.0 - (i - last) as f64 / 3.0).max(0.0));
        previous_energy += (*energy - previous_energy) * 0.28;
        let at_ms = i as u64 * 100;
        let drop_hit = drop_ms.is_some_and(|d| at_ms >= d && at_ms < d.saturating_add(300));
        let before_drop = drop_ms.is_some_and(|d| at_ms < d && at_ms.saturating_add(700) >= d);
        let zoom = 1.0
            + strength
                * (0.025
                    + 0.10 * envelope
                    + 0.035 * accent_envelope
                    + if drop_hit { 0.04 } else { 0.0 });
        let w = ((width as f64 / zoom) as u32 / 2 * 2).max(2);
        let h = ((height as f64 / zoom) as u32 / 2 * 2).max(2);
        let wiggle = envelope * strength * 0.8;
        let x = ((width - w) as f64 * (0.5 + 0.5 * wiggle * ((i as f64 * 2.4).sin()))) as u32;
        let y = ((height - h) as f64 * (0.5 + 0.5 * wiggle * ((i as f64 * 1.7).cos()))) as u32;
        writeln!(
            commands,
            "{:.3} crop w {}, crop h {}, crop x {}, crop y {};",
            i as f64 / 10.0,
            w,
            h,
            x,
            y
        )
        .unwrap();
        if let Some(o) = options {
            // Short accents with a 400ms cooldown avoid a constant strobe.
            let flash = o.flash && (accent || drop_hit && at_ms < drop_ms.unwrap() + 100);
            let brightness = if before_drop {
                -0.13 * strength
            } else if flash {
                0.09 * strength
            } else {
                0.0
            };
            let color = if flash { 0.28 * strength } else { 0.0 };
            let shift = if o.glitch && (accent && accents % 3 == 0 || drop_hit) {
                (width as f64 * 0.014 * strength).round() as i32
            } else {
                0
            };
            let channel = if o.flash_color == "red" { "rm" } else { "gm" };
            writeln!(commands, "{:.3} eq@phonk brightness {:.4}, colorbalance@phonk {} {:.4}, rgbashift@phonk rh {}, rgbashift@phonk bh {};", i as f64 / 10.0, brightness, channel, color, shift, -shift).unwrap();
        }
    }
    let command_file =
        std::env::temp_dir().join(format!("blyacut-visualizer-{}.txt", blyacut_core::id()));
    std::fs::write(&command_file, commands).map_err(|e| e.to_string())?;
    // UUID-only temporary basename: no user-supplied text enters the filter expression.
    let mut filter = format!(
        "sendcmd=f='{}',crop={width}:{height}:0:0,scale={width}:{height}:eval=frame,setsar=1",
        command_file.display()
    );
    if let Some(o) = options {
        match o.look.as_str() {
            "neon" => {
                filter.push_str(",eq=saturation=1.45:contrast=1.08,colorbalance=rm=0.08:bm=0.1")
            }
            "vhs" => filter.push_str(",eq=saturation=0.72:contrast=0.92:brightness=0.02"),
            "warehouse" => filter.push_str(",eq=saturation=0.65:contrast=1.15:brightness=-0.07"),
            _ => {}
        }
        filter.push_str(",eq@phonk=brightness=0,colorbalance@phonk=gm=0,rgbashift@phonk=rh=0:bh=0");
        if o.grain {
            filter.push_str(&format!(
                ",noise=alls={}:allf=t+u,drawgrid=w=iw:h=4:t=1:c=black@0.04",
                (2.0 + 5.0 * strength).round() as u32
            ));
        }
    }
    let waveform = options.is_some_and(|o| o.waveform);
    let graph = if waveform {
        let ww = (width * 8 / 10 / 2 * 2).max(2);
        let wh = (height / 10 / 2 * 2).max(2);
        let color = match options.map(|o| o.look.as_str()) {
            Some("neon") => "0xff33cc",
            Some("vhs") => "0xffbd69",
            Some("warehouse") => "0xff4433",
            _ => "0x81ff33",
        };
        format!("[0:v]{filter}[base];[0:a:0]aformat=channel_layouts=mono,showwaves=s={ww}x{wh}:mode=line:draw=full:rate=30:colors={color},format=rgba,colorkey=0x000000:0.12:0.08,colorchannelmixer=aa=0.8[wave];[base][wave]overlay=(W-w)/2:H-h-H*0.035:shortest=0:eof_action=pass[vout]")
    } else {
        format!("[0:v]{filter}[vout]")
    };
    let log_path =
        std::env::temp_dir().join(format!("blyacut-visualizer-{}.log", blyacut_core::id()));
    let result = (|| -> Result<()> {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled before visualizer export".into());
        }
        let log = std::fs::File::create(&log_path).map_err(|e| e.to_string())?;
        let mut child = Command::new(binary("ffmpeg"))
            .args([
                "-nostdin",
                "-v",
                "error",
                "-n",
                "-protocol_whitelist",
                "file,pipe",
                "-i",
            ])
            .arg(&asset.path)
            .args([
                "-map",
                "[vout]",
                "-map",
                "0:a:0",
                "-map",
                "0:s?",
                "-filter_complex",
                &graph,
                "-c:v",
                "libx264",
                "-preset",
                "fast",
                "-crf",
                "18",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "copy",
                "-c:s",
                "copy",
                "-map_metadata",
                "0",
                "-movflags",
                "+faststart",
            ])
            .arg(output)
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(|e| format!("FFmpeg export: {e}"))?;
        loop {
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Cancelled; a partial visualizer output may remain".into());
            }
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(status) if status.success() => return Ok(()),
                Some(_) => {
                    return Err(format!(
                        "Visualizer export failed (a partial output may remain): {}",
                        std::fs::read_to_string(&log_path).unwrap_or_default()
                    ))
                }
                None => std::thread::sleep(Duration::from_millis(100)),
            }
        }
    })();
    let _ = std::fs::remove_file(command_file);
    let _ = std::fs::remove_file(log_path);
    match result {
        Ok(()) => Ok(VisualizerResult {
            analysis_windows: energies.len(),
            detected_accents: accents,
            drop_ms,
            drop_estimated: options.is_some_and(|o| o.drop && o.drop_ms.is_none())
                && drop_ms.is_some(),
        }),
        Err(e) => Err(e),
    }
}
