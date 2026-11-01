//! External processes always receive argument vectors, never shell strings.
pub mod loopmix;
pub mod music;
pub mod release;
pub mod studio;
pub mod visualizer;
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
/// Piecewise-linear FFmpeg expression over `var` through `(seconds, value)` points.
fn piecewise(points: &[(f64, f64)], var: &str) -> String {
    let (_, last_v) = points[points.len() - 1];
    let mut expr = format!("{last_v:.5}");
    for pair in points.windows(2).rev() {
        let ((t0, v0), (t1, v1)) = (pair[0], pair[1]);
        expr = format!(
            "if(lt({var},{t1:.4}),{v0:.5}+({:.5})*({var}-{t0:.4})/{:.4},{expr})",
            v1 - v0,
            (t1 - t0).max(0.0001)
        );
    }
    let (t0, v0) = points[0];
    format!("if(lt({var},{t0:.4}),{v0:.5},{expr})")
}
/// Keyframe values of one transform field, as (clip-local seconds, value) points.
fn key_points(
    clip: &blyacut_core::Clip,
    field: impl Fn(&blyacut_core::Transform) -> f64,
) -> Option<Vec<(f64, f64)>> {
    let points: Vec<(f64, f64)> = clip
        .keyframes
        .iter()
        .map(|k| (k.at_ms as f64 / 1000.0, field(&k.transform)))
        .collect();
    let varies = points.windows(2).any(|p| (p[0].1 - p[1].1).abs() > 1e-9);
    varies.then_some(points)
}
/// Filter chain for one overlay input: fit the frame, apply its scale, effects and opacity.
/// Keyframed scale and opacity are evaluated per frame on the clip's own clock.
pub fn overlay_layer(project: &Project, clip: &blyacut_core::Clip) -> String {
    let s = &project.settings;
    let t = clip.transform_at(0);
    let fit = format!(
        "fps={}/{},scale={w}:{h}:force_original_aspect_ratio=decrease",
        s.fps_num,
        s.fps_den,
        w = s.width,
        h = s.height
    );
    let scale = match key_points(clip, |t| t.scale_milli as f64 / 1000.0) {
        Some(points) => {
            // `scale` re-evaluates per frame but its `t` stays at the first frame; after the
            // fps filter the frame index gives the clip-local clock exactly.
            let clock = format!("(n*{}/{})", s.fps_den, s.fps_num);
            let k = piecewise(&points, &clock);
            format!("scale=w='trunc(iw*({k})/2)*2':h='trunc(ih*({k})/2)*2':eval=frame")
        }
        None => {
            let k = t.scale_milli as f64 / 1000.0;
            format!("scale=trunc(iw*{k:.3}/2)*2:trunc(ih*{k:.3}/2)*2")
        }
    };
    let mut chain = format!("{fit},setsar=1");
    let effects = video_effect_filters(clip);
    if !effects.is_empty() {
        chain = format!("{chain},format=yuv420p,{effects}");
    }
    let opacity = match key_points(clip, |t| t.opacity_milli as f64 / 1000.0) {
        Some(points) => format!(
            "geq=lum='lum(X,Y)':cb='cb(X,Y)':cr='cr(X,Y)':a='alpha(X,Y)*({})'",
            piecewise(&points, "T")
        ),
        None => format!(
            "colorchannelmixer=aa={:.3}",
            t.opacity_milli as f64 / 1000.0
        ),
    };
    // Opacity first: `geq` keeps its first frame size, so the size may only change after it.
    format!("{chain},format=yuva420p,{opacity},{scale},setsar=1")
}
/// `overlay` position: the clip centre lands at the transform's fractional point.
/// Keyframed positions follow the timeline clock from the clip's start.
pub fn overlay_position(clip: &blyacut_core::Clip) -> String {
    let t = clip.transform_at(0);
    let start = clip.position_ms as f64 / 1000.0;
    let axis =
        |field: fn(&blyacut_core::Transform) -> f64, fixed: f64| match key_points(clip, field) {
            Some(points) => {
                let shifted: Vec<(f64, f64)> =
                    points.iter().map(|(at, v)| (at + start, *v)).collect();
                format!("'{}'", piecewise(&shifted, "t"))
            }
            None => format!("{fixed:.3}"),
        };
    let x = axis(|t| t.x_milli as f64 / 1000.0, t.x_milli as f64 / 1000.0);
    let y = axis(|t| t.y_milli as f64 / 1000.0, t.y_milli as f64 / 1000.0);
    format!("x=main_w*{x}-overlay_w/2:y=main_h*{y}-overlay_h/2:eval=frame")
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
        let read = if asset.still {
            clip.duration()
        } else {
            clip.span()
        };
        args.splice(at..at, ["-t".to_string(), sec(read)]);
        let (from, to) = (clip.position_ms, clip.position_ms + clip.duration());
        let timing = video_timing(clip)
            .map(|t| format!("{t},"))
            .unwrap_or_default();
        graph.push(format!(
            "[{input}:v]{timing}{},setpts=PTS-STARTPTS+{}/TB[o{input}]",
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
/// With `settings.fill` the source is scaled up and centre-cropped instead (no bars).
pub(crate) fn normalize_filter(project: &Project) -> String {
    let s = &project.settings;
    let frame = if s.fill {
        format!(
            "scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}",
            w = s.width,
            h = s.height
        )
    } else {
        format!(
            "scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2",
            w = s.width,
            h = s.height
        )
    };
    format!(
        "{frame},setsar=1,fps={}/{},format=yuv420p",
        s.fps_num, s.fps_den
    )
}
/// Picture retiming for a clip: `reverse` then `setpts` for speed (before normalization).
pub fn video_timing(clip: &blyacut_core::Clip) -> Option<String> {
    let mut parts = vec![];
    if clip.reverse {
        parts.push("reverse".to_string());
    }
    if clip.speed_milli != 1000 {
        parts.push(format!(
            "setpts=PTS*{:.6}",
            1000.0 / clip.speed_milli as f64
        ));
    }
    (!parts.is_empty()).then(|| parts.join(","))
}
/// Sound retiming: `areverse`, then `atempo` steps kept inside its 0.5–2.0 sweet spot.
pub fn audio_timing(clip: &blyacut_core::Clip) -> Option<String> {
    let mut parts = vec![];
    if clip.reverse {
        parts.push("areverse".to_string());
    }
    let mut tempo = clip.speed_milli as f64 / 1000.0;
    while tempo > 2.0 {
        parts.push("atempo=2".into());
        tempo /= 2.0;
    }
    while tempo < 0.5 {
        parts.push("atempo=0.5".into());
        tempo /= 0.5;
    }
    if (tempo - 1.0).abs() > 1e-6 {
        parts.push(format!("atempo={tempo:.6}"));
    }
    (!parts.is_empty()).then(|| parts.join(","))
}
/// Insert `-t <source span>` before the first `-i` so retimed inputs read only their range.
fn limit_first_input(args: &mut Vec<String>, clip: &blyacut_core::Clip) {
    if let Some(i) = args.iter().position(|a| a == "-i") {
        args.splice(i..i, ["-t".to_string(), sec(clip.span())]);
    }
}
/// Main-track clip segment: normalized picture with effects, linked audio with its filters.
fn clip_args(
    project: &Project,
    asset: &Asset,
    clip: &blyacut_core::Clip,
    out: &Path,
) -> Vec<String> {
    let mut args = segment_args(project, asset, clip.source_start_ms, clip.duration(), out);
    if !asset.still {
        limit_first_input(&mut args, clip);
    }
    if let Some(i) = args.iter().position(|a| a == "-af") {
        let timing = audio_timing(clip)
            .map(|t| format!("{t},"))
            .unwrap_or_default();
        args[i + 1] = format!("aresample=48000,{timing}apad,{}", audio_filter(clip));
    }
    if let (Some(timing), Some(i)) = (video_timing(clip), args.iter().position(|a| a == "-vf")) {
        args[i + 1] = format!("{timing},{}", args[i + 1]);
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
}
/// One segment of length D centred on the cut: the outgoing clip runs past its end, the
/// incoming one starts early (handles), blended with `xfade` and `acrossfade`.
fn transition_args(
    project: &Project,
    (a_asset, a): (&Asset, &blyacut_core::Clip),
    (b_asset, b): (&Asset, &blyacut_core::Clip),
    transition: &blyacut_core::Transition,
    out: &Path,
) -> Vec<String> {
    let (before, _) = blyacut_core::transition_halves(transition);
    let d = transition.duration_ms as u64;
    let mut args: Vec<String> = [
        "-nostdin",
        "-v",
        "error",
        "-n",
        "-protocol_whitelist",
        "file,pipe",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut add = |asset: &Asset, start: u64| {
        let mut input = media_input(project, asset, start);
        let at = input.len() - 2;
        input.splice(at..at, ["-t".to_string(), sec(d)]);
        args.extend(input);
    };
    add(a_asset, a.source_end_ms.saturating_sub(before));
    // Stills have unlimited handles: their single frame loops.
    add(b_asset, b.source_start_ms.saturating_sub(before));
    let audio = |asset: &Asset, video_input: usize, next: &mut usize| {
        if asset.has_audio {
            format!("{video_input}:a:0")
        } else {
            *next += 1;
            format!("{}:a:0", *next - 1)
        }
    };
    let mut next = 2;
    let (aa, ab) = (audio(a_asset, 0, &mut next), audio(b_asset, 1, &mut next));
    for _ in 2..next {
        args.extend([
            "-f".into(),
            "lavfi".into(),
            "-t".into(),
            sec(d),
            "-i".into(),
            "anullsrc=r=48000:cl=stereo".into(),
        ]);
    }
    // Linked-audio effects and gain apply; clip fades belong to the clip bodies.
    let sound = |c: &blyacut_core::Clip| {
        let mut c = c.clone();
        c.fade_in_ms = 0;
        c.fade_out_ms = 0;
        audio_filter(&c)
    };
    let picture = |c: &blyacut_core::Clip| {
        let effects = video_effect_filters(c);
        let base = normalize_filter(project);
        if effects.is_empty() {
            format!("{base},settb=AVTB")
        } else {
            format!("{base},{effects},format=yuv420p,settb=AVTB")
        }
    };
    let graph = format!(
        "[0:v]{}[va];[1:v]{}[vb];[va][vb]xfade=transition={}:duration={}:offset=0,format=yuv420p[v];\
         [{aa}]aresample=48000,aformat=channel_layouts=stereo,apad,atrim=end={},{}[xa];\
         [{ab}]aresample=48000,aformat=channel_layouts=stereo,apad,atrim=end={},{}[xb];\
         [xa][xb]acrossfade=d={}[a]",
        picture(a),
        picture(b),
        transition.kind.name(),
        sec(d),
        sec(d),
        sound(a),
        sec(d),
        sound(b),
        sec(d)
    );
    args.extend([
        "-filter_complex".into(),
        graph,
        "-map".into(),
        "[v]".into(),
        "-map".into(),
        "[a]".into(),
        "-t".into(),
        sec(d),
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
    args
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
    let mut encode = |args: &dyn Fn(&Path) -> Vec<String>, duration: u64| -> Result<()> {
        let name = format!("segment-{segment_index}.mp4");
        segment_index += 1;
        let path = scratch.0.join(&name);
        run(&args(&path), &cancel, &log, |ms| {
            progress((done + ms.min(duration)) as f64 / end as f64 * 0.75)
        })?;
        list.push_str(&format!("file '{name}'\n"));
        done += duration;
        Ok(())
    };
    let asset_of = |clip: &blyacut_core::Clip| -> Result<&Asset> {
        let asset = project
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .ok_or("Unknown asset")?;
        if !asset.path.is_absolute() || !asset.path.is_file() {
            return Err(format!("Missing media: {}", asset.path.display()));
        }
        Ok(asset)
    };
    let mut cursor = 0;
    for (i, clip) in clips.iter().enumerate() {
        let transition = clip.transition_in.filter(|_| i > 0);
        if transition.is_none() && clip.position_ms > cursor {
            let gap = clip.position_ms - cursor;
            encode(&|path| black_args(project, gap, path), gap)?;
        }
        let asset = asset_of(clip)?;
        if let Some(t) = &transition {
            let prev = &clips[i - 1];
            let prev_asset = asset_of(prev)?;
            encode(
                &|path| transition_args(project, (prev_asset, prev), (asset, clip), t, path),
                t.duration_ms as u64,
            )?;
        }
        // The clip body excludes the parts its transitions render.
        let after_in = transition
            .map(|t| blyacut_core::transition_halves(&t).1)
            .unwrap_or(0);
        let before_out = clips
            .get(i + 1)
            .and_then(|n| n.transition_in)
            .map(|t| blyacut_core::transition_halves(&t).0)
            .unwrap_or(0);
        let mut body = clip.clone();
        body.source_start_ms += after_in;
        body.source_end_ms -= before_out;
        body.fade_in_ms = body.fade_in_ms.saturating_sub(after_in);
        body.fade_out_ms = body.fade_out_ms.saturating_sub(before_out);
        let length = body.duration();
        encode(&|path| clip_args(project, asset, &body, path), length)?;
        cursor = clip.position_ms + clip.duration();
    }
    if end > cursor {
        let tail = end - cursor;
        encode(&|path| black_args(project, tail, path), tail)?;
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
            sec(c.span()),
            "-i".into(),
            a.path.to_string_lossy().into(),
        ]);
        let timing = audio_timing(c).map(|t| format!("{t},")).unwrap_or_default();
        filters.push(format!("[{}:a:0]asetpts=PTS-STARTPTS,aresample=48000,aformat=channel_layouts=stereo,{timing}{},atrim=duration={},adelay={}|{}[a{}]",i+1,audio_filter(c),sec(c.duration()),c.position_ms,c.position_ms,i));
    }
    let sum = |labels: &[String], out: &str| {
        if labels.len() == 1 {
            format!("{}anull[{out}]", labels[0])
        } else {
            format!(
                "{}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0[{out}]",
                labels.concat(),
                labels.len()
            )
        }
    };
    let ducked: Vec<usize> = (0..clips.len()).filter(|&i| clips[i].duck_db > 0).collect();
    let mut inputs: Vec<String> = vec!["[base]".into()];
    if ducked.is_empty() {
        inputs.extend((0..clips.len()).map(|i| format!("[a{i}]")));
    } else {
        // Voice bus = main-video sound + non-ducking clips; it keys a compressor on each music clip.
        let voice: Vec<String> = std::iter::once("[base]".to_string())
            .chain(
                (0..clips.len())
                    .filter(|i| !ducked.contains(i))
                    .map(|i| format!("[a{i}]")),
            )
            .collect();
        filters.push(sum(&voice, "voice"));
        let keys: String = (0..ducked.len()).map(|k| format!("[key{k}]")).collect();
        filters.push(format!(
            "[voice]asplit={}[voicemix]{keys}",
            ducked.len() + 1
        ));
        inputs = vec!["[voicemix]".into()];
        for (k, &i) in ducked.iter().enumerate() {
            // Any audible voice drives the compressor almost to silence; `mix` keeps the dry
            // share 10^(-duck/20), so the depth is ≈duck_db whatever the voice level.
            let dry = 10f64.powf(-(clips[i].duck_db as f64) / 20.0);
            filters.push(format!(
                "[a{i}][key{k}]sidechaincompress=threshold=0.00178:ratio=20:attack=20:release=250:knee=2:makeup=1:mix={:.4}[d{i}]",
                1.0 - dry
            ));
            inputs.push(format!("[d{i}]"));
        }
    }
    filters.push(format!(
        "{}amix=inputs={}:duration=first:dropout_transition=0:normalize=0,alimiter=limit=1:level=0:latency=1[mixed]",
        inputs.concat(),
        inputs.len()
    ));
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
