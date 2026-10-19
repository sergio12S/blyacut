//! External processes always receive argument vectors, never shell strings.
pub mod studio;
use blyacut_core::{id, Asset, Project, Result, TrackKind};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
const STILL_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
pub fn binary(name: &str) -> PathBuf {
    let key = if name == "ffmpeg" {
        "BLYACUT_FFMPEG"
    } else {
        "BLYACUT_FFPROBE"
    };
    if let Some(p) = std::env::var_os(key) {
        return p.into();
    }
    for root in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        let p = Path::new(root).join(name);
        if p.is_file() {
            return p;
        }
    }
    name.into()
}
pub fn probe(path: &Path) -> Result<Asset> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("Media unavailable: {e}"))?;
    if !path.is_file() {
        return Err("Select a regular media file".into());
    }
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ![
        "mp4", "mov", "m4v", "mkv", "webm", "wav", "mp3", "m4a", "flac", "ogg",
    ]
    .contains(&ext.as_str())
        && !STILL_EXTENSIONS.contains(&ext.as_str())
    {
        return Err(
            "Supported media: MP4, MOV, M4V, MKV, WebM, WAV, MP3, M4A, FLAC, OGG, PNG, JPEG, WebP"
                .into(),
        );
    }
    let still = STILL_EXTENSIONS.contains(&ext.as_str());
    let out = Command::new(binary("ffprobe"))
        .args([
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-show_format",
            "-show_streams",
            "-of",
            "json",
        ])
        .arg(&path)
        .output()
        .map_err(|e| format!("FFprobe unavailable. Install FFmpeg or set BLYACUT_FFPROBE: {e}"))?;
    if !out.status.success() {
        return Err(format!("FFprobe: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let streams = v["streams"].as_array().ok_or("No streams")?;
    let video = streams
        .iter()
        .find(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"] != 1);
    let audio = streams.iter().find(|s| s["codec_type"] == "audio");
    let main = video.or(audio).ok_or("No video or audio stream")?;
    if still {
        let video = video.ok_or("Image has no picture")?;
        let (width, height) = (
            video["width"].as_u64().unwrap_or(0) as u32,
            video["height"].as_u64().unwrap_or(0) as u32,
        );
        if width == 0 || height == 0 || width > 16384 || height > 16384 {
            return Err("Unsupported image dimensions".into());
        }
        return Ok(Asset {
            id: id(),
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            path,
            duration_ms: blyacut_core::STILL_MS,
            width,
            height,
            has_audio: false,
            still: true,
        });
    }
    let seconds = v["format"]["duration"]
        .as_str()
        .or_else(|| main["duration"].as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .ok_or("Unknown media duration")?;
    if !seconds.is_finite() || seconds <= 0.0 || seconds > 604800.0 {
        return Err("Invalid media duration (maximum one week)".into());
    }
    let width = video.and_then(|v| v["width"].as_u64()).unwrap_or(0) as u32;
    let height = video.and_then(|v| v["height"].as_u64()).unwrap_or(0) as u32;
    if video.is_some() && (width == 0 || height == 0 || width > 16384 || height > 16384) {
        return Err("Unsupported media dimensions".into());
    }
    Ok(Asset {
        id: id(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        path,
        duration_ms: (seconds * 1000.0).round() as u64,
        width,
        height,
        has_audio: streams.iter().any(|s| s["codec_type"] == "audio"),
        still: false,
    })
}
/// Input arguments for one source: stills loop one frame, media seek to `start`.
pub(crate) fn media_input(project: &Project, asset: &Asset, start: u64) -> Vec<String> {
    let path = asset.path.to_string_lossy().to_string();
    if asset.still {
        let s = &project.settings;
        vec![
            "-loop".into(),
            "1".into(),
            "-framerate".into(),
            format!("{}/{}", s.fps_num, s.fps_den),
            "-i".into(),
            path,
        ]
    } else {
        vec!["-ss".into(), sec(start), "-i".into(), path]
    }
}
/// Filter chain for one overlay input: fit the frame, apply its scale, effects and opacity.
pub fn overlay_layer(project: &Project, clip: &blyacut_core::Clip) -> String {
    let s = &project.settings;
    let t = clip.transform.unwrap_or_default();
    let scale = t.scale_milli as f64 / 1000.0;
    let mut chain = format!(
        "fps={}/{},scale={w}:{h}:force_original_aspect_ratio=decrease,scale=trunc(iw*{scale:.3}/2)*2:trunc(ih*{scale:.3}/2)*2,setsar=1",
        s.fps_num,
        s.fps_den,
        w = s.width,
        h = s.height
    );
    let effects = video_effect_filters(clip);
    if !effects.is_empty() {
        chain = format!("{chain},format=yuv420p,{effects}");
    }
    format!(
        "{chain},format=yuva420p,colorchannelmixer=aa={:.3}",
        t.opacity_milli as f64 / 1000.0
    )
}
/// `overlay` position: the clip centre lands at the transform's fractional point.
pub fn overlay_position(clip: &blyacut_core::Clip) -> String {
    let t = clip.transform.unwrap_or_default();
    format!(
        "x=main_w*{:.3}-overlay_w/2:y=main_h*{:.3}-overlay_h/2",
        t.x_milli as f64 / 1000.0,
        t.y_milli as f64 / 1000.0
    )
}
/// Composite overlay clips (in track order) onto the rendered main track.
fn overlay_args(
    project: &Project,
    overlays: &[(&blyacut_core::Clip, &Asset)],
    base: &Path,
    out: &Path,
) -> Vec<String> {
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
    for (i, (clip, asset)) in overlays.iter().enumerate() {
        args.extend(media_input(project, asset, clip.source_start_ms));
        let input = i + 1;
        // -t applies to the input that follows; stills need it to end.
        let at = args.len() - 2;
        args.splice(at..at, ["-t".to_string(), sec(clip.duration())]);
        let (from, to) = (clip.position_ms, clip.position_ms + clip.duration());
        graph.push(format!(
            "[{input}:v]{},setpts=PTS-STARTPTS+{}/TB[o{input}]",
            overlay_layer(project, clip),
            sec(from)
        ));
        graph.push(format!(
            "[{current}][o{input}]overlay={}:eof_action=pass:enable='between(t,{},{})'[v{input}]",
            overlay_position(clip),
            sec(from),
            sec(to)
        ));
        current = format!("v{input}");
    }
    graph.push(format!("[{current}]format=yuv420p[vout]"));
    args.extend([
        "-filter_complex".into(),
        graph.join(";"),
        "-map".into(),
        "[vout]".into(),
        "-map".into(),
        "0:a".into(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "veryfast".into(),
        "-crf".into(),
        "20".into(),
        "-c:a".into(),
        "copy".into(),
        "-movflags".into(),
        "+faststart".into(),
        "-progress".into(),
        "pipe:1".into(),
        out.to_string_lossy().into(),
    ]);
    args
}
/// Letterbox any source into the project frame, rate and pixel format.
pub(crate) fn normalize_filter(project: &Project) -> String {
    let s = &project.settings;
    format!(
        "scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={}/{},format=yuv420p",
        s.fps_num,
        s.fps_den,
        w = s.width,
        h = s.height
    )
}
pub(crate) fn sec(ms: u64) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}
pub fn segment_args(
    project: &Project,
    asset: &Asset,
    start: u64,
    duration: u64,
    out: &Path,
) -> Vec<String> {
    let mut a = vec![
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
        "-n".into(),
        "-protocol_whitelist".into(),
        "file,pipe".into(),
    ];
    a.extend(media_input(project, asset, start));
    if !asset.has_audio {
        a.extend([
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            "anullsrc=r=48000:cl=stereo".into(),
        ]);
    }
    a.extend([
        "-t".into(),
        sec(duration),
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        if asset.has_audio { "0:a:0" } else { "1:a:0" }.into(),
        "-vf".into(),
        normalize_filter(project),
        "-af".into(),
        "aresample=48000,apad".into(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "veryfast".into(),
        "-crf".into(),
        "20".into(),
        "-c:a".into(),
        "aac".into(),
        "-ar".into(),
        "48000".into(),
        "-ac".into(),
        "2".into(),
        "-video_track_timescale".into(),
        "90000".into(),
        "-progress".into(),
        "pipe:1".into(),
        out.to_string_lossy().into(),
    ]);
    a
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn run(
    args: &[String],
    cancel: &AtomicBool,
    log: &Path,
    mut on_time: impl FnMut(u64),
) -> Result<()> {
    let err = std::fs::File::create(log).map_err(|e| e.to_string())?;
    let mut child = Command::new(binary("ffmpeg"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(err)
        .spawn()
        .map_err(|e| format!("FFmpeg unavailable. Install FFmpeg or set BLYACUT_FFMPEG: {e}"))?;
    let stdout = child.stdout.take().ok_or("Missing FFmpeg stdout")?;
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if let Some(value) = line
                .strip_prefix("out_time_us=")
                .and_then(|s| s.parse::<u64>().ok())
            {
                if tx.send(value / 1000).is_err() {
                    break;
                }
            }
        }
    });
    loop {
        for ms in rx.try_iter() {
            on_time(ms);
        }
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err("Export cancelled".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let _ = reader.join();
                for ms in rx.try_iter() {
                    on_time(ms);
                }
                if status.success() {
                    return Ok(());
                }
                return Err(format!(
                    "FFmpeg failed: {}",
                    std::fs::read_to_string(log).unwrap_or_default()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(e.to_string());
            }
        }
    }
}
/// Snapshot export. Output is published only after successful encoding; never overwrites a file.
pub fn render(
    project: &Project,
    output: &Path,
    cancel: Arc<AtomicBool>,
    mut progress: impl FnMut(f64),
) -> Result<()> {
    project.validate()?;
    if output.extension().and_then(|s| s.to_str()) != Some("mp4") {
        return Err("Output must end in .mp4".into());
    }
    if output.exists() {
        return Err("Output already exists; choose a new filename".into());
    }
    let clips = project
        .main_video_track()
        .map(|t| t.clips.as_slice())
        .unwrap_or(&[]);
    let mut overlays = vec![];
    for track in project
        .tracks
        .iter()
        .filter(|t| project.is_overlay_track(t))
    {
        for clip in &track.clips {
            let asset = project
                .assets
                .iter()
                .find(|a| a.id == clip.asset_id)
                .ok_or("Unknown asset")?;
            if !asset.path.is_absolute() || !asset.path.is_file() {
                return Err(format!("Missing media: {}", asset.path.display()));
            }
            overlays.push((clip, asset));
        }
    }
    if overlays.len() > 32 {
        return Err("Export supports up to 32 overlay clips".into());
    }
    let audio_clips: Vec<_> = project
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Audio)
        .flat_map(|t| &t.clips)
        .collect();
    if audio_clips.len() > 64 {
        return Err("Export supports up to 64 audio clips".into());
    }
    let end = project.duration();
    if end == 0 {
        return Err("Timeline is empty".into());
    }
    let parent = output
        .parent()
        .ok_or("Output needs a directory")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let scratch = Scratch(parent.join(format!(".blyacut-render-{}", id())));
    std::fs::create_dir(&scratch.0).map_err(|e| e.to_string())?;
    let log = scratch.0.join("ffmpeg.log");
    let mut list = String::new();
    let mut done = 0;
    let mut segment_index = 0;
    let mut encode =
        |asset: Option<&Asset>, clip: Option<&blyacut_core::Clip>, duration: u64| -> Result<()> {
            let name = format!("segment-{segment_index}.mp4");
            segment_index += 1;
            let path = scratch.0.join(&name);
            let args = if let (Some(asset), Some(clip)) = (asset, clip) {
                if !asset.path.is_absolute() || !asset.path.is_file() {
                    return Err(format!("Missing media: {}", asset.path.display()));
                }
                let mut args = segment_args(project, asset, clip.source_start_ms, duration, &path);
                if let Some(i) = args.iter().position(|a| a == "-af") {
                    args[i + 1] = format!("aresample=48000,apad,{}", audio_filter(clip));
                }
                let visual = video_effect_filters(clip);
                if !visual.is_empty() {
                    if let Some(i) = args.iter().position(|a| a == "-vf") {
                        args[i + 1].push(',');
                        args[i + 1].push_str(&visual);
                        args[i + 1].push_str(",format=yuv420p");
                    }
                }
                args
            } else {
                black_args(project, duration, &path)
            };
            run(&args, &cancel, &log, |ms| {
                progress((done + ms.min(duration)) as f64 / end as f64 * 0.75)
            })?;
            list.push_str(&format!("file '{name}'\n"));
            done += duration;
            Ok(())
        };
    let mut cursor = 0;
    for clip in clips {
        if clip.position_ms > cursor {
            encode(None, None, clip.position_ms - cursor)?;
        }
        let asset = project
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .ok_or("Unknown asset")?;
        encode(Some(asset), Some(clip), clip.duration())?;
        cursor = clip.position_ms + clip.duration();
    }
    if end > cursor {
        encode(None, None, end - cursor)?;
    }
    std::fs::write(scratch.0.join("concat.txt"), list).map_err(|e| e.to_string())?;
    let base = scratch.0.join("base.mp4");
    let result = scratch.0.join("result.mp4");
    let args = vec![
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
        "-n".into(),
        "-f".into(),
        "concat".into(),
        "-safe".into(),
        "1".into(),
        "-i".into(),
        scratch.0.join("concat.txt").to_string_lossy().into(),
        "-c".into(),
        "copy".into(),
        "-movflags".into(),
        "+faststart".into(),
        base.to_string_lossy().into(),
    ];
    run(&args, &cancel, &log, |_| {})?;
    progress(0.8);
    let base = if overlays.is_empty() {
        base
    } else {
        let layered = scratch.0.join("layered.mp4");
        let args = overlay_args(project, &overlays, &base, &layered);
        run(&args, &cancel, &log, |ms| {
            progress(0.8 + ms.min(end) as f64 / end as f64 * 0.1)
        })?;
        layered
    };
    progress(0.9);
    if audio_clips.is_empty() {
        std::fs::rename(&base, &result).map_err(|e| e.to_string())?;
    } else {
        let args = mix_args(project, &audio_clips, &base, &result)?;
        run(&args, &cancel, &log, |ms| {
            progress(0.9 + ms.min(end) as f64 / end as f64 * 0.09)
        })?;
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Export cancelled".into());
    }
    std::fs::hard_link(&result, output).map_err(|e| format!("Could not publish export: {e}"))?;
    progress(1.0);
    Ok(())
}

/// Shared envelope semantics: linear gain, fades measured from the clip boundaries.
pub fn audio_filter(clip: &blyacut_core::Clip) -> String {
    let mut effects: Vec<String> = clip
        .effects
        .iter()
        .flat_map(|r| &r.steps)
        .filter(|s| !s.is_video())
        .map(|s| step_filter(s, clip))
        .collect();
    effects.push(format!("volume={:.3}", clip.gain_milli as f64 / 1000.0));
    let mut filter = effects.join(",");
    if clip.fade_in_ms > 0 {
        filter.push_str(&format!(",afade=t=in:st=0:d={}", sec(clip.fade_in_ms)));
    }
    if clip.fade_out_ms > 0 {
        filter.push_str(&format!(
            ",afade=t=out:st={}:d={}",
            sec(clip.duration() - clip.fade_out_ms),
            sec(clip.fade_out_ms)
        ));
    }
    filter
}
fn black_args(project: &Project, duration: u64, out: &Path) -> Vec<String> {
    let s = &project.settings;
    vec![
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
        "-n".into(),
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        format!(
            "color=c=black:s={}x{}:r={}/{}",
            s.width, s.height, s.fps_num, s.fps_den
        ),
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        "anullsrc=r=48000:cl=stereo".into(),
        "-t".into(),
        sec(duration),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "veryfast".into(),
        "-crf".into(),
        "20".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        "-ar".into(),
        "48000".into(),
        "-ac".into(),
        "2".into(),
        "-video_track_timescale".into(),
        "90000".into(),
        "-progress".into(),
        "pipe:1".into(),
        out.to_string_lossy().into(),
    ]
}
pub fn mix_args(
    project: &Project,
    clips: &[&blyacut_core::Clip],
    base: &Path,
    out: &Path,
) -> Result<Vec<String>> {
    let mut args = vec![
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
        "-n".into(),
        "-i".into(),
        base.to_string_lossy().into(),
    ];
    let mut filters = vec!["[0:a]asetpts=PTS-STARTPTS[base]".to_string()];
    let mut inputs = String::from("[base]");
    for (i, c) in clips.iter().enumerate() {
        let a = project
            .assets
            .iter()
            .find(|a| a.id == c.asset_id)
            .ok_or("Unknown audio asset")?;
        if !a.path.is_absolute() || !a.path.is_file() {
            return Err(format!("Missing media: {}", a.path.display()));
        }
        args.extend([
            "-protocol_whitelist".into(),
            "file,pipe".into(),
            "-ss".into(),
            sec(c.source_start_ms),
            "-t".into(),
            sec(c.duration()),
            "-i".into(),
            a.path.to_string_lossy().into(),
        ]);
        filters.push(format!("[{}:a:0]asetpts=PTS-STARTPTS,aresample=48000,aformat=channel_layouts=stereo,{},atrim=duration={},adelay={}|{}[a{}]",i+1,audio_filter(c),sec(c.duration()),c.position_ms,c.position_ms,i));
        inputs.push_str(&format!("[a{i}]"));
    }
    filters.push(format!("{}amix=inputs={}:duration=first:dropout_transition=0:normalize=0,alimiter=limit=1:level=0:latency=1[mixed]",inputs,clips.len()+1));
    args.extend([
        "-filter_complex".into(),
        filters.join(";"),
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "[mixed]".into(),
        "-c:v".into(),
        "copy".into(),
        "-c:a".into(),
        "aac".into(),
        "-t".into(),
        sec(project.duration()),
        "-movflags".into(),
        "+faststart".into(),
        "-progress".into(),
        "pipe:1".into(),
        out.to_string_lossy().into(),
    ]);
    Ok(args)
}

/// Validated recipe values are compiled into fixed operators; no external filter strings.
pub fn video_effect_filters(clip: &blyacut_core::Clip) -> String {
    clip.effects
        .iter()
        .flat_map(|r| &r.steps)
        .filter(|s| s.is_video())
        .map(|s| step_filter(s, clip))
        .collect::<Vec<_>>()
        .join(",")
}

/// One fixed FFmpeg operator per validated step. Only integers from the recipe are formatted in.
pub fn step_filter(step: &blyacut_core::effects::EffectStep, clip: &blyacut_core::Clip) -> String {
    use blyacut_core::effects::EffectStep::*;
    let milli = |v: f64| v / 1000.0;
    match step {
        Brightness { gain_milli } => {
            let g = milli(*gain_milli as f64);
            format!("colorchannelmixer=rr={g:.3}:gg={g:.3}:bb={g:.3}")
        }
        Contrast { gain_milli } => format!("eq=contrast={:.3}", milli(*gain_milli as f64)),
        Saturation { gain_milli } => format!("eq=saturation={:.3}", milli(*gain_milli as f64)),
        Hue { degrees } => format!("hue=h={degrees}"),
        Gamma { gamma_milli } => format!("eq=gamma={:.3}", milli(*gamma_milli as f64)),
        Vibrance { intensity_milli } => {
            format!("vibrance=intensity={:.3}", milli(*intensity_milli as f64))
        }
        Temperature { kelvin } => format!("colortemperature=temperature={kelvin}"),
        Blur { strength } => format!("gblur=sigma={strength}"),
        Sharpen { amount_milli } => format!("unsharp=5:5:{:.3}", milli(*amount_milli as f64)),
        Vignette { strength_milli } => format!(
            "vignette=angle={:.4}",
            std::f64::consts::FRAC_PI_2 * milli(*strength_milli as f64)
        ),
        FilmGrain { strength } => format!("noise=alls={strength}:allf=t+u"),
        Pixelate { block } => format!("pixelize=w={block}:h={block}"),
        Posterize { levels } => {
            let q = 256.0 / *levels as f64;
            format!("lutrgb=r=trunc(val/{q:.3})*{q:.3}:g=trunc(val/{q:.3})*{q:.3}:b=trunc(val/{q:.3})*{q:.3}")
        }
        ChromaticAberration { pixels } => format!("rgbashift=rh=-{pixels}:bh={pixels}"),
        GhostTrails { decay_milli } => format!("lagfun=decay={:.3}", milli(*decay_milli as f64)),
        FadeFromBlack { duration_ms } => {
            format!(
                "fade=t=in:st=0:d={}",
                sec((*duration_ms as u64).min(clip.duration()))
            )
        }
        FadeToBlack { duration_ms } => {
            let d = (*duration_ms as u64).min(clip.duration());
            format!("fade=t=out:st={}:d={}", sec(clip.duration() - d), sec(d))
        }
        Grayscale {} => "hue=s=0".into(),
        Sepia {} => "colorchannelmixer=.393:.769:.189:0:.349:.686:.168:0:.272:.534:.131".into(),
        Invert {} => "negate".into(),
        Mirror {} => "hflip".into(),
        Flip {} => "vflip".into(),
        EdgeDetect {} => "edgedetect=mode=wires".into(),
        LowPass { frequency_hz } => format!("lowpass=f={frequency_hz}:t=q:w=0.70710678:p=2"),
        HighPass { frequency_hz } => format!("highpass=f={frequency_hz}:t=q:w=0.70710678:p=2"),
        Echo {
            delay_ms,
            decay_milli,
        } => format!(
            "aecho=1:1:{delay_ms}:{:.3}",
            milli(*decay_milli as f64).max(0.001)
        ),
        Bass { gain_db } => format!("bass=g={gain_db}"),
        Treble { gain_db } => format!("treble=g={gain_db}"),
        Equalizer {
            frequency_hz,
            gain_db,
        } => format!("equalizer=f={frequency_hz}:t=q:w=1:g={gain_db}"),
        Reverb { room_milli } => {
            let r = milli(*room_milli as f64);
            format!(
                "aecho=0.8:0.9:{:.0}|{:.0}|{:.0}:{:.3}|{:.3}|{:.3}",
                20.0 + 60.0 * r,
                40.0 + 110.0 * r,
                60.0 + 160.0 * r,
                0.2 + 0.3 * r,
                0.15 + 0.25 * r,
                0.1 + 0.2 * r
            )
        }
        Compressor {
            threshold_db,
            ratio,
        } => format!(
            "acompressor=threshold={:.6}:ratio={ratio}",
            10f64.powf(*threshold_db as f64 / 20.0)
        ),
        Pitch { semitones } => {
            let f = 2f64.powf(*semitones as f64 / 12.0);
            format!(
                "aresample=48000,asetrate={:.0},aresample=48000,atempo={:.6}",
                48000.0 * f,
                1.0 / f
            )
        }
        Tremolo {
            frequency_milli,
            depth_milli,
        } => format!(
            "tremolo=f={:.3}:d={:.3}",
            milli(*frequency_milli as f64),
            milli(*depth_milli as f64).max(0.001)
        ),
        Vibrato {
            frequency_milli,
            depth_milli,
        } => format!(
            "vibrato=f={:.3}:d={:.3}",
            milli(*frequency_milli as f64),
            milli(*depth_milli as f64)
        ),
        AutoPan { frequency_milli } => {
            format!("apulsator=hz={:.3}", milli(*frequency_milli as f64))
        }
        Bitcrush { bits } => format!("acrusher=bits={bits}:mode=log:aa=1"),
        Denoise { strength_db } => format!("afftdn=nr={strength_db}"),
        StereoWiden { amount_milli } => {
            format!("extrastereo=m={:.3}", milli(*amount_milli as f64))
        }
        Chorus {} => "chorus=0.5:0.9:50|60|40:0.4|0.32|0.3:0.25|0.4|0.3:2|2.3|1.3".into(),
        Flanger {} => "flanger".into(),
        Phaser {} => "aphaser".into(),
        Robot {} => "afftfilt=real='hypot(re,im)':imag='0':win_size=512:overlap=0.75".into(),
        Normalize {} => "dynaudnorm".into(),
    }
}
