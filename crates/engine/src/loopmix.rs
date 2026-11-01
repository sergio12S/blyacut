//! Build an exact-length caption-free video mix with crossfades at every join and loop seam.
use crate::{probe, release::run_video};
use blyacut_core::Result;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub path: PathBuf,
    pub start_ms: u64,
    pub end_ms: u64,
}
#[derive(Clone)]
pub struct Request {
    pub segments: Vec<Segment>,
    pub output: PathBuf,
    pub duration_seconds: u32,
    pub fade_ms: u32,
    pub width: u32,
    pub height: u32,
}
#[derive(Serialize)]
pub struct MixResult {
    pub path: PathBuf,
    pub duration_seconds: u32,
    pub cycle_seconds: f64,
    pub fade_ms: u32,
    pub warnings: Vec<String>,
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn seconds(frames: u64) -> f64 {
    frames as f64 / 30.0
}

pub fn validate(request: &Request) -> Result<()> {
    if request.segments.is_empty() || request.segments.len() > 8 {
        return Err("Provide 1–8 video segments".into());
    }
    if !(10..=3600).contains(&request.duration_seconds) || !(300..=5000).contains(&request.fade_ms)
    {
        return Err("duration_seconds must be 10–3600 and fade_ms 300–5000".into());
    }
    if request.width < 160
        || request.height < 160
        || request.width > 1920
        || request.height > 1920
        || !request.width.is_multiple_of(2)
        || !request.height.is_multiple_of(2)
        || u64::from(request.width) * u64::from(request.height) > 1920 * 1080
    {
        return Err("Width and height must be even, within 160–1920 pixels, with at most 1920×1080 pixels total".into());
    }
    if request.output.exists() || request.output.extension().and_then(|s| s.to_str()) != Some("mp4")
    {
        return Err("Output must be a new .mp4 file".into());
    }
    let fade_frames = (u64::from(request.fade_ms) * 30 + 500) / 1000;
    let mut frames = 0_u64;
    for segment in &request.segments {
        let span = segment
            .end_ms
            .checked_sub(segment.start_ms)
            .ok_or("Segment end must be after start")?;
        if span > 60_000 {
            return Err(
                "Each segment must be at most 60 seconds and longer than twice the crossfade"
                    .into(),
            );
        }
        let n = (span * 30 + 500) / 1000;
        if n <= 2 * fade_frames {
            return Err("Each segment must be longer than twice the crossfade".into());
        }
        let asset = probe(&segment.path)?;
        if asset.still || asset.width == 0 || !asset.has_audio || segment.end_ms > asset.duration_ms
        {
            return Err("Each segment must fit a video with audio".into());
        }
        frames += n;
    }
    let cycle_frames = frames - (request.segments.len() as u64) * fade_frames;
    if cycle_frames <= fade_frames || u64::from(request.duration_seconds) * 30 <= 2 * fade_frames {
        return Err("Mix or cycle is too short for that crossfade".into());
    }
    Ok(())
}

/// Stages: create a lossless-audio loop cycle; repeat it and crossfade the final boundary.
pub fn render(request: &Request, cancel: &AtomicBool, progress: impl Fn(u8)) -> Result<MixResult> {
    validate(request)?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled before export".into());
    }
    let fade_frames = (u64::from(request.fade_ms) * 30 + 500) / 1000;
    let fade = seconds(fade_frames);
    let durations: Vec<u64> = request
        .segments
        .iter()
        .map(|s| ((s.end_ms - s.start_ms) * 30 + 500) / 1000)
        .collect();
    let mut base_frames = durations[0];
    let scratch =
        Scratch(std::env::temp_dir().join(format!("blyacut-loopmix-{}", blyacut_core::id())));
    std::fs::create_dir(&scratch.0).map_err(|e| e.to_string())?;
    let cycle = scratch.0.join("cycle.mkv");
    let mut args: Vec<String> = [
        "-nostdin",
        "-v",
        "error",
        "-n",
        "-protocol_whitelist",
        "file,pipe",
    ]
    .iter()
    .map(|x| x.to_string())
    .collect();
    let mut graph = Vec::new();
    for (i, (segment, frames)) in request.segments.iter().zip(&durations).enumerate() {
        args.extend([
            "-ss".into(),
            format!("{:.3}", segment.start_ms as f64 / 1000.0),
            "-t".into(),
            format!("{:.6}", seconds(*frames)),
            "-i".into(),
            segment.path.to_string_lossy().into(),
        ]);
        let (w, h) = (request.width, request.height);
        graph.push(format!("[{i}:v]fps=30,split=2[b{i}][f{i}];[b{i}]scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},boxblur=12:1,eq=brightness=-0.20[bg{i}];[f{i}]scale={w}:{h}:force_original_aspect_ratio=decrease[fg{i}];[bg{i}][fg{i}]overlay=(W-w)/2:(H-h)/2,format=yuv420p,setsar=1,tpad=stop_mode=clone:stop_duration=0.1,trim=end_frame={frames},settb=1/30,setpts=N[v{i}];[{i}:a]aresample=48000,aformat=sample_fmts=fltp:channel_layouts=stereo,apad,atrim=duration={:.6},asetpts=PTS-STARTPTS[a{i}]",seconds(*frames)));
    }
    let mut video = "v0".to_string();
    let mut audio = "a0".to_string();
    for (i, frames) in durations.iter().enumerate().skip(1) {
        let offset = seconds(base_frames - fade_frames);
        graph.push(format!("[{video}][v{i}]xfade=transition=fade:duration={fade:.6}:offset={offset:.6}[vx{i}];[{audio}][a{i}]acrossfade=d={fade:.6}:c1=tri:c2=tri[ax{i}]"));
        video = format!("vx{i}");
        audio = format!("ax{i}");
        base_frames += frames - fade_frames;
    }
    let seam_offset = seconds(base_frames - 2 * fade_frames);
    let fade_samples = fade_frames * 1600;
    let head_end_frame = fade_frames + 1;
    let head_end_sample = fade_samples + 1;
    // Include the first frame/sample of the next cycle at the end of the incoming fade.
    // That makes the encoded boundary meet the following cycle at the same source instant.
    graph.push(format!("[{video}]split=2[vmain][vhead];[vmain]trim=start_frame={fade_frames},setpts=PTS-STARTPTS[vm];[vhead]trim=start_frame=1:end_frame={head_end_frame},setpts=PTS-STARTPTS[vh];[vm][vh]xfade=transition=fade:duration={fade:.6}:offset={seam_offset:.6}[v];[{audio}]asplit=2[amain][ahead];[amain]atrim=start_sample={fade_samples},asetpts=PTS-STARTPTS[am];[ahead]atrim=start_sample=1:end_sample={head_end_sample},asetpts=PTS-STARTPTS[ah];[am][ah]acrossfade=d={fade:.6}:c1=tri:c2=tri[a]"));
    args.extend([
        "-filter_complex".into(),
        graph.join(";"),
        "-map".into(),
        "[v]".into(),
        "-map".into(),
        "[a]".into(),
        "-sn".into(),
        "-dn".into(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "veryfast".into(),
        "-crf".into(),
        "18".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "flac".into(),
        cycle.to_string_lossy().into(),
    ]);
    progress(0);
    run_video(&args, cancel, &scratch.0)?;
    progress(1);
    let duration = request.duration_seconds as f64;
    let video_end = u64::from(request.duration_seconds) * 30 + fade_frames;
    let audio_end = u64::from(request.duration_seconds) * 48000 + fade_samples;
    // A five-millisecond edge ramp suppresses AAC boundary clicks after encoding.
    let graph=format!("[0:v]trim=start_frame={fade_frames}:end_frame={video_end},setpts=PTS-STARTPTS,fps=30,settb=1/30[mainv];[1:v]trim=start_frame=1:end_frame={head_end_frame},setpts=PTS-STARTPTS,fps=30,settb=1/30[headv];[mainv][headv]xfade=transition=fade:duration={fade:.6}:offset={:.6},format=yuv420p[v];[0:a]atrim=start_sample={fade_samples}:end_sample={audio_end},asetpts=PTS-STARTPTS[maina];[1:a]atrim=start_sample=1:end_sample={head_end_sample},asetpts=PTS-STARTPTS[heada];[maina][heada]acrossfade=d={fade:.6}:c1=tri:c2=tri,afade=t=in:st=0:d=0.005,afade=t=out:st={:.3}:d=0.005[a]",duration-fade,duration-0.005);
    let final_args = vec![
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
        "-n".into(),
        "-protocol_whitelist".into(),
        "file,pipe".into(),
        "-stream_loop".into(),
        "-1".into(),
        "-i".into(),
        cycle.to_string_lossy().into(),
        "-i".into(),
        cycle.to_string_lossy().into(),
        "-filter_complex".into(),
        graph,
        "-map".into(),
        "[v]".into(),
        "-map".into(),
        "[a]".into(),
        "-sn".into(),
        "-dn".into(),
        "-t".into(),
        request.duration_seconds.to_string(),
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
        "-b:a".into(),
        "256k".into(),
        "-movflags".into(),
        "+faststart".into(),
        request.output.to_string_lossy().into(),
    ];
    run_video(&final_args, cancel, &scratch.0)?;
    progress(2);
    Ok(MixResult{path:request.output.clone(),duration_seconds:request.duration_seconds,cycle_seconds:seconds(base_frames-fade_frames),fade_ms:(fade*1000.0).round() as u32,warnings:vec!["Crossfades smooth the edits but do not automatically match tempo, key or musical phrases; review joins by ear.".into(),"Subtitle streams are omitted, but text already burned into input pixels cannot be removed.".into()]})
}
