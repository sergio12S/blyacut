//! A real agent session: stdio JSON-RPC against `blyacut-mcp --headless`, no desktop app.
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

struct Agent {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    id: u64,
}
impl Agent {
    fn start(dir: &std::path::Path, autonomous: bool) -> Self {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_blyacut-mcp"));
        cmd.arg("--headless").arg(dir);
        if autonomous {
            cmd.arg("--autonomous");
        }
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut agent = Agent {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
            id: 0,
        };
        let init = agent.rpc("initialize", json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}));
        assert!(init["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("Headless"));
        writeln!(
            agent.input,
            "{}",
            json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )
        .unwrap();
        agent
    }
    fn rpc(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        writeln!(
            self.input,
            "{}",
            json!({"jsonrpc":"2.0","id":self.id,"method":method,"params":params})
        )
        .unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
    fn call(&mut self, tool: &str, args: Value) -> Result<Value, String> {
        let r = self.rpc("tools/call", json!({"name":tool,"arguments":args}));
        let result = &r["result"];
        if result["isError"] == true {
            Err(result["content"][0]["text"].as_str().unwrap().to_string())
        } else {
            Ok(result["structuredContent"].clone())
        }
    }
    fn ok(&mut self, tool: &str, args: Value) -> Value {
        self.call(tool, args)
            .unwrap_or_else(|e| panic!("{tool}: {e}"))
    }
}

fn ffmpeg(args: &[&str]) {
    assert!(Command::new(blyacut_engine::binary("ffmpeg"))
        .args(["-v", "error"])
        .args(args)
        .status()
        .unwrap()
        .success());
}

#[test]
fn agent_renders_bass_visualizer_from_real_audio() {
    let dir = std::env::temp_dir()
        .join(format!("blyacut-bass-{}", blyacut_core::id()))
        .canonicalize_or_create();
    let p = |name: &str| dir.join(name).to_string_lossy().to_string();
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=black:s=160x120:r=10:d=2,drawbox=x=20:y=10:w=35:h=75:color=white:t=fill",
        "-f",
        "lavfi",
        "-i",
        "aevalsrc=if(lt(t\\,1)\\,0\\,0.7*sin(2*PI*80*t)):s=8000:d=2",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        &p("input.mp4"),
    ]);
    let mut agent = Agent::start(&dir, true);
    let listing = agent.ok("list_music_inputs", json!({"folder":p("")}));
    assert!(listing["paths"]
        .as_array()
        .unwrap()
        .iter()
        .any(|path| path.as_str() == Some(p("input.mp4").as_str())));
    assert!(agent
        .call("list_music_inputs", json!({"folder":"/etc"}))
        .is_err());
    assert!(agent
        .call(
            "render_bass_visualizer",
            json!({"input_path":p("input.mp4"),"output_path":p("bad.mp4"),"intensity":101})
        )
        .is_err());
    assert!(agent
        .call(
            "render_bass_visualizer",
            json!({"input_path":"/etc/passwd","output_path":p("bad.mp4")})
        )
        .is_err());
    let result = agent.ok(
        "render_bass_visualizer",
        json!({"input_path":p("input.mp4"),"output_path":p("out.mp4"),"intensity":100}),
    );
    assert!(result["analysis_windows"].as_u64().unwrap() >= 20);
    assert!(agent
        .call(
            "render_bass_visualizer",
            json!({"input_path":p("input.mp4"),"output_path":p("out.mp4")})
        )
        .is_err());
    let frame = |time: &str| {
        Command::new(blyacut_engine::binary("ffmpeg"))
            .args([
                "-v",
                "error",
                "-ss",
                time,
                "-i",
                &p("out.mp4"),
                "-frames:v",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-",
            ])
            .output()
            .unwrap()
            .stdout
    };
    let quiet = frame("0.5");
    let bass = frame("1.5");
    assert_eq!(quiet.len(), 160 * 120 * 3);
    assert_eq!(bass.len(), quiet.len());
    assert!(
        quiet
            .iter()
            .zip(&bass)
            .filter(|(a, b)| a.abs_diff(**b) > 20)
            .count()
            > 300,
        "bass must visibly alter the static image"
    );
    let source = blyacut_engine::probe(&dir.join("input.mp4")).unwrap();
    let output = blyacut_engine::probe(&dir.join("out.mp4")).unwrap();
    assert!(output.has_audio && source.duration_ms.abs_diff(output.duration_ms) < 150);
    assert!(agent.call("render_dirty_phonk", json!({"input_path":p("input.mp4"),"output_path":p("invalid.mp4"),"options":{"flash_color":"blue"}})).is_err());
    let phonk = agent.ok("render_dirty_phonk", json!({"input_path":p("input.mp4"),"output_path":p("phonk.mp4"),"intensity":80,"options":{"drop_ms":1200,"flash_color":"red"}}));
    assert_eq!(phonk["analysis"]["drop_ms"], 1200);
    assert_eq!(phonk["analysis"]["drop_estimated"], false);
    assert!(phonk["analysis"]["detected_accents"].as_u64().unwrap() > 0);
    let rendered = blyacut_engine::probe(&dir.join("phonk.mp4")).unwrap();
    assert!(rendered.has_audio && source.duration_ms.abs_diff(rendered.duration_ms) < 150);
    for (name, wave) in [("wave.mp4", true), ("no-wave.mp4", false)] {
        agent.ok("render_dirty_phonk", json!({"input_path":p("input.mp4"),"output_path":p(name),"intensity":100,"options":{"waveform":wave,"flash":false,"grain":false,"glitch":false,"drop":false}}));
    }
    let lower_frame = |name: &str| {
        Command::new(blyacut_engine::binary("ffmpeg"))
            .args([
                "-v",
                "error",
                "-ss",
                "1.5",
                "-i",
                &p(name),
                "-frames:v",
                "1",
                "-vf",
                "crop=160:30:0:90",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-",
            ])
            .output()
            .unwrap()
            .stdout
    };
    let with_wave = lower_frame("wave.mp4");
    let without_wave = lower_frame("no-wave.mp4");
    assert_eq!(with_wave.len(), 160 * 30 * 3);
    assert!(
        with_wave
            .iter()
            .zip(&without_wave)
            .filter(|(a, b)| a.abs_diff(**b) > 12)
            .count()
            > 20,
        "waveform must visibly change the lower part of the frame"
    );
    assert!(
        with_wave
            .as_chunks::<3>()
            .0
            .iter()
            .zip(without_wave.as_chunks::<3>().0.iter())
            .filter(|(a, b)| a[1] > a[0].saturating_add(15) && a[1] > b[1].saturating_add(15))
            .count()
            > 5,
        "waveform should be green"
    );
    agent.ok("render_dirty_phonk", json!({"input_path":p("input.mp4"),"output_path":p("flash.mp4"),"intensity":100,"options":{"flash_color":"red","flash":true,"waveform":false,"grain":false,"glitch":false,"drop":false}}));
    let frame_at_hit = |name: &str| {
        Command::new(blyacut_engine::binary("ffmpeg"))
            .args([
                "-v",
                "error",
                "-ss",
                "1.0",
                "-i",
                &p(name),
                "-frames:v",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-",
            ])
            .output()
            .unwrap()
            .stdout
    };
    let flashed = frame_at_hit("flash.mp4");
    let plain = frame_at_hit("no-wave.mp4");
    assert_eq!(flashed.len(), plain.len());
    assert!(
        flashed
            .iter()
            .zip(&plain)
            .filter(|(a, b)| a.abs_diff(**b) > 8)
            .count()
            > 100,
        "bass onset should trigger a visible flash"
    );
    drop(agent);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn agent_creates_release_package_with_timed_subtitles() {
    let dir = std::env::temp_dir()
        .join(format!("blyacut-release-test-{}", blyacut_core::id()))
        .canonicalize_or_create();
    let p = |name: &str| dir.join(name).to_string_lossy().to_string();
    std::fs::write(p("text.srt"), "1\n00:00:01,000 --> 00:00:02,000\nПерший рядок\n\n2\n00:00:02,000 --> 00:00:04,000\nДругий рядок\n\n3\n00:00:06,000 --> 00:00:09,000\nТретій рядок\n\n4\n00:00:11,000 --> 00:00:14,000\nЧетвертий рядок\n").unwrap();
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:s=160x120:r=10:d=15",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=80:duration=15",
        "-i",
        &p("text.srt"),
        "-map",
        "0:v",
        "-map",
        "1:a",
        "-map",
        "2:s",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-c:s",
        "mov_text",
        &p("source.mp4"),
    ]);
    let mut agent = Agent::start(&dir, true);
    assert!(agent.call("render_release_package",json!({"input_path":p("source.mp4"),"output_dir":p(""),"prefix":"../bad","title":"Track","short_seconds":5})).is_err());
    assert!(agent.call("render_release_package",json!({"input_path":p("source.mp4"),"output_dir":p(""),"prefix":"invalid","title":"Track","short_seconds":5,"starts_ms":[0,5000,15000]})).is_err());
    let pack=agent.ok("render_release_package",json!({"input_path":p("source.mp4"),"output_dir":p(""),"prefix":"test","title":"Track","short_seconds":5}));
    assert_eq!(pack["caption_source"], "embedded subtitles");
    assert_eq!(pack["shorts"].as_array().unwrap().len(), 3);
    assert_eq!(pack["shorts"][0]["start_ms"], 0);
    assert_eq!(pack["shorts"][1]["start_ms"], 5000);
    assert_eq!(pack["shorts"][2]["start_ms"], 10000);
    let full = blyacut_engine::probe(&dir.join("test-full.mp4")).unwrap();
    assert_eq!((full.width, full.height), (1920, 1080));
    for i in 1..=3 {
        let s = blyacut_engine::probe(&dir.join(format!("test-short-{i}.mp4"))).unwrap();
        assert_eq!((s.width, s.height), (720, 1280));
        assert!(s.has_audio && s.duration_ms.abs_diff(5000) < 150);
        assert!(pack["shorts"][i - 1]["captions"].as_u64().unwrap() > 0);
    }
    let frame = |time: &str| {
        Command::new(blyacut_engine::binary("ffmpeg"))
            .args([
                "-v",
                "error",
                "-ss",
                time,
                "-i",
                &p("test-short-1.mp4"),
                "-frames:v",
                "1",
                "-vf",
                "crop=720:300:0:750",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-",
            ])
            .output()
            .unwrap()
            .stdout
    };
    let before = frame("0.5");
    let during = frame("2.0");
    let after = frame("2.1");
    assert_eq!(before.len(), 720 * 300 * 3);
    assert!(
        before
            .iter()
            .zip(&during)
            .filter(|(a, b)| a.abs_diff(**b) > 20)
            .count()
            > 1000,
        "timed caption must appear over the static video"
    );
    assert!(
        during
            .iter()
            .zip(&after)
            .filter(|(a, b)| a.abs_diff(**b) > 20)
            .count()
            < 200,
        "adjacent captions must not overlap at their boundary"
    );
    assert!(agent.call("render_release_package",json!({"input_path":p("source.mp4"),"output_dir":p(""),"prefix":"test","title":"Track","short_seconds":5})).is_err());
    assert!(agent.call("render_release_package",json!({"input_path":p("source.mp4"),"output_dir":p(""),"prefix":"invalid-none","title":"Track","short_seconds":5,"caption_mode":"none","captions_srt":"unused"})).is_err());
    let clean=agent.ok("render_release_package",json!({"input_path":p("source.mp4"),"output_dir":p(""),"prefix":"clean","title":"Track","short_seconds":5,"caption_mode":"none"}));
    assert_eq!(clean["caption_source"], "none");
    assert!(clean["shorts"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| s["captions"] == 0));
    let probe = Command::new(blyacut_engine::binary("ffprobe"))
        .args([
            "-v",
            "error",
            "-show_streams",
            "-of",
            "json",
            &p("clean-full.mp4"),
        ])
        .output()
        .unwrap();
    let streams: Value = serde_json::from_slice(&probe.stdout).unwrap();
    assert!(!streams["streams"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["codec_type"] == "subtitle"));
    let clean_frame = Command::new(blyacut_engine::binary("ffmpeg"))
        .args([
            "-v",
            "error",
            "-ss",
            "2",
            "-i",
            &p("clean-short-1.mp4"),
            "-frames:v",
            "1",
            "-vf",
            "crop=720:300:0:750",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .output()
        .unwrap()
        .stdout;
    assert_eq!(clean_frame.len(), 720 * 300 * 3);
    assert!(
        clean_frame
            .as_chunks::<3>()
            .0
            .iter()
            .filter(|p| p[0] > 200 && p[1] > 200 && p[2] > 200)
            .count()
            < 100,
        "no-caption mode must not burn text"
    );
    let candidates = agent.ok(
        "analyze_music_candidates",
        json!({"input_path":p("source.mp4"),"seconds":5}),
    );
    assert_eq!(candidates["candidates"].as_array().unwrap().len(), 3);
    assert_eq!(candidates["candidates"][1]["start_ms"], 5000);
    let previews=agent.ok("render_music_previews",json!({"input_path":p("source.mp4"),"output_dir":p(""),"prefix":"preview","start_ms":0,"seconds":3}));
    assert_eq!(previews["paths"].as_array().unwrap().len(), 3);
    for look in ["dirty", "neon", "vhs"] {
        assert!(dir.join(format!("preview-{look}.mp4")).is_file());
    }
    let thumbs=agent.ok("create_music_thumbnails",json!({"input_path":p("source.mp4"),"output_dir":p(""),"prefix":"cover","title":"Тестовий трек","times_ms":[1000,6000,11000]}));
    assert_eq!(thumbs["paths"].as_array().unwrap().len(), 3);
    assert!(agent.call("start_release_jobs",json!({"jobs":[{"input_path":p("source.mp4"),"output_dir":p(""),"prefix":"badwords","title":"Track","short_seconds":5,"captions_srt":"1\n00:00:01,000 --> 00:00:02,000\ntwo words\n","caption_mode":"word"}]})).is_err());
    let srt="1\n00:00:01,000 --> 00:00:04,000\nChecked\n\n2\n00:00:06,000 --> 00:00:09,000\nSecond\n\n3\n00:00:11,000 --> 00:00:14,000\nFinal\n";
    let job1 = json!({"input_path":p("source.mp4"),"output_dir":p(""),"prefix":"queue-one","title":"Track","short_seconds":5,"captions_srt":srt,"caption_mode":"word","look":"neon"});
    let job2 = json!({"input_path":p("source.mp4"),"output_dir":p(""),"prefix":"queue-two","title":"Track","short_seconds":5});
    let submitted = agent.ok("start_release_jobs", json!({"jobs":[job1,job2]}));
    let id1 = submitted["jobs"][0]["id"].as_str().unwrap().to_string();
    let id2 = submitted["jobs"][1]["id"].as_str().unwrap().to_string();
    agent.ok("cancel_music_job", json!({"job_id":id2}));
    let mut finished = false;
    for _ in 0..300 {
        let jobs = agent.ok("get_music_jobs", json!({}));
        let entries = jobs["jobs"].as_array().unwrap();
        let first = entries.iter().find(|j| j["id"] == id1).unwrap();
        let second = entries.iter().find(|j| j["id"] == id2).unwrap();
        assert_eq!(second["status"], "cancelled");
        if first["status"] == "failed" {
            panic!("background job failed: {first}");
        }
        if first["status"] == "complete" {
            assert_eq!(first["completed_steps"], 5);
            assert_eq!(first["total_steps"], 5);
            assert!(dir.join("queue-one-styled.mp4").is_file());
            assert_eq!(first["result"]["caption_source"], "provided SRT");
            finished = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(finished, "background job did not finish");
    let word_frame = Command::new(blyacut_engine::binary("ffmpeg"))
        .args([
            "-v",
            "error",
            "-ss",
            "2",
            "-i",
            &p("queue-one-short-1.mp4"),
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .output()
        .unwrap()
        .stdout;
    assert!(
        word_frame
            .as_chunks::<3>()
            .0
            .iter()
            .filter(|p| p[1] > 160 && p[1] > p[2].saturating_add(35))
            .count()
            > 100,
        "current word should appear highlighted"
    );
    assert!(!dir.join("queue-two-full.mp4").exists());
    let running=agent.ok("start_release_jobs",json!({"jobs":[{"input_path":p("source.mp4"),"output_dir":p(""),"prefix":"cancel-running","title":"Track","short_seconds":5,"look":"warehouse"}]}));
    let running_id = running["jobs"][0]["id"].as_str().unwrap().to_string();
    let mut encoder_started = false;
    for _ in 0..1000 {
        let jobs = agent.ok("get_music_jobs", json!({}));
        let status = jobs["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["id"] == running_id)
            .unwrap()["status"]
            .as_str()
            .unwrap()
            .to_string();
        if status == "running" && dir.join("cancel-running-styled.mp4").exists() {
            encoder_started = true;
            break;
        }
        assert_ne!(status, "failed", "job failed before cancellation");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(encoder_started, "styled encoder did not start");
    agent.ok("cancel_music_job", json!({"job_id":running_id}));
    let mut cancelled = false;
    for _ in 0..100 {
        let jobs = agent.ok("get_music_jobs", json!({}));
        let status = jobs["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["id"] == running_id)
            .unwrap()["status"]
            .as_str()
            .unwrap()
            .to_string();
        if status == "cancelled" {
            cancelled = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(cancelled, "running job should stop");
    drop(agent);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn agent_creates_caption_free_loop_with_smooth_boundary() {
    let dir = std::env::temp_dir()
        .join(format!("blyacut-loop-{}", blyacut_core::id()))
        .canonicalize_or_create();
    let p = |name: &str| dir.join(name).to_string_lossy().to_string();
    std::fs::write(
        p("captions.srt"),
        "1\n00:00:00,000 --> 00:00:03,000\nSHOULD NOT APPEAR\n",
    )
    .unwrap();
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=s=320x180:r=30:d=4,negate=enable='eq(mod(n,3),2)'",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000:duration=4",
        "-i",
        &p("captions.srt"),
        "-map",
        "0:v",
        "-map",
        "1:a",
        "-map",
        "2:s",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-c:s",
        "mov_text",
        &p("one.mp4"),
    ]);
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=red:s=320x180:r=30:d=4",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=660:sample_rate=48000:duration=4",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        &p("two.mp4"),
    ]);
    let mut agent = Agent::start(&dir, true);
    let segments = json!([{"path":p("one.mp4"),"start_ms":0,"end_ms":4000},{"path":p("two.mp4"),"start_ms":0,"end_ms":4000}]);
    assert!(agent.call("start_loop_mix",json!({"segments":[{"path":"/etc/passwd","start_ms":0,"end_ms":4000}],"output_path":p("bad.mp4")})).is_err());
    assert!(agent.call("start_loop_mix",json!({"segments":[{"path":p("one.mp4"),"start_ms":0,"end_ms":u64::MAX}],"output_path":p("bad.mp4")})).is_err());
    assert!(agent
        .call(
            "start_loop_mix",
            json!({"segments":segments,"output_path":p("bad.mp4"),"fade_ms":3000})
        )
        .is_err());
    let queued=agent.ok("start_loop_mix",json!({"segments":segments,"output_path":p("loop.mp4"),"duration_seconds":17,"fade_ms":1000,"width":320,"height":180}));
    let id = queued["job"]["id"].as_str().unwrap().to_string();
    let mut done = false;
    for _ in 0..300 {
        let jobs = agent.ok("get_music_jobs", json!({}));
        let j = jobs["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["id"] == id)
            .unwrap();
        if j["status"] == "failed" {
            panic!("loop failed: {j}");
        }
        if j["status"] == "complete" {
            assert_eq!(j["completed_steps"], 2);
            assert_eq!(j["result"]["duration_seconds"], 17);
            done = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(done, "loop job did not complete");
    let meta = Command::new(blyacut_engine::binary("ffprobe"))
        .args([
            "-v",
            "error",
            "-show_streams",
            "-of",
            "json",
            &p("loop.mp4"),
        ])
        .output()
        .unwrap();
    let info: Value = serde_json::from_slice(&meta.stdout).unwrap();
    assert!(!info["streams"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["codec_type"] == "subtitle"));
    let asset = blyacut_engine::probe(&dir.join("loop.mp4")).unwrap();
    assert!(asset.has_audio && asset.duration_ms.abs_diff(17000) < 40);
    let frame = |at: &str| {
        Command::new(blyacut_engine::binary("ffmpeg"))
            .args([
                "-v",
                "error",
                "-ss",
                at,
                "-i",
                &p("loop.mp4"),
                "-frames:v",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "gray",
                "-",
            ])
            .output()
            .unwrap()
            .stdout
    };
    let first = frame("0");
    let last = frame("16.95");
    let next = frame("0.0333");
    assert_eq!(first.len(), 320 * 180);
    assert_eq!(last.len(), first.len());
    let diff = |a: &[u8], b: &[u8]| {
        a.iter()
            .zip(b)
            .map(|(x, y)| x.abs_diff(*y) as f64)
            .sum::<f64>()
            / a.len() as f64
    };
    assert!(
        diff(&first, &last) < diff(&first, &next) * 1.5 + 5.0,
        "loop video boundary has a hard visual jump"
    );
    let pcm = Command::new(blyacut_engine::binary("ffmpeg"))
        .args([
            "-v",
            "error",
            "-i",
            &p("loop.mp4"),
            "-map",
            "0:a:0",
            "-ac",
            "1",
            "-ar",
            "48000",
            "-f",
            "f32le",
            "-",
        ])
        .output()
        .unwrap()
        .stdout;
    let samples: Vec<f32> = pcm
        .as_chunks::<4>()
        .0
        .iter()
        .map(|x| f32::from_le_bytes(*x))
        .collect();
    assert_eq!(samples.len(), 17 * 48000);
    assert!(
        (samples[0] - samples[samples.len() - 1]).abs() < 0.05,
        "audio loop seam has a large discontinuity"
    );
    assert!(agent.call("start_loop_mix",json!({"segments":segments,"output_path":p("loop.mp4"),"duration_seconds":17,"width":320,"height":180})).is_err());
    let long=agent.ok("start_loop_mix",json!({"segments":[{"path":p("two.mp4"),"start_ms":0,"end_ms":4000}],"output_path":p("ten-minutes.mp4"),"duration_seconds":600,"fade_ms":700,"width":160,"height":160}));
    let long_id = long["job"]["id"].as_str().unwrap().to_string();
    let mut completed = false;
    for _ in 0..1200 {
        let jobs = agent.ok("get_music_jobs", json!({}));
        let j = jobs["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["id"] == long_id)
            .unwrap();
        if j["status"] == "failed" {
            panic!("10-minute loop failed: {j}");
        }
        if j["status"] == "complete" {
            completed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(completed, "10-minute loop did not finish");
    let ten = blyacut_engine::probe(&dir.join("ten-minutes.mp4")).unwrap();
    assert!(
        ten.has_audio && ten.duration_ms.abs_diff(600_000) < 40,
        "wrong 10-minute duration: {}",
        ten.duration_ms
    );
    let scan = Command::new(blyacut_engine::binary("ffmpeg"))
        .args([
            "-hide_banner",
            "-i",
            &p("ten-minutes.mp4"),
            "-vn",
            "-af",
            "silencedetect=noise=-50dB:d=0.2",
            "-f",
            "null",
            "-",
        ])
        .output()
        .unwrap();
    assert!(scan.status.success());
    assert!(
        !String::from_utf8_lossy(&scan.stderr).contains("silence_start"),
        "repeated loop should not introduce audible gaps"
    );
    let cancel_args = json!({"segments":[{"path":p("two.mp4"),"start_ms":0,"end_ms":4000}],"output_path":p("cancel-loop.mp4"),"duration_seconds":600,"fade_ms":700,"width":160,"height":160});
    let pending = agent.ok("start_loop_mix", cancel_args.clone());
    let cancel_id = pending["job"]["id"].as_str().unwrap().to_string();
    assert!(
        agent.call("start_loop_mix", cancel_args).is_err(),
        "a queued or existing output must not be reused"
    );
    let mut encoding = false;
    for _ in 0..1000 {
        let state = agent.ok("get_music_jobs", json!({}));
        let j = state["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["id"] == cancel_id)
            .unwrap();
        if j["status"] == "failed" {
            panic!("loop failed before cancellation: {j}");
        }
        if j["status"] == "running" && dir.join("cancel-loop.mp4").exists() {
            encoding = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(encoding, "final loop encoder did not start");
    agent.ok("cancel_music_job", json!({"job_id":cancel_id}));
    let mut cancelled = false;
    for _ in 0..100 {
        let state = agent.ok("get_music_jobs", json!({}));
        let j = state["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["id"] == cancel_id)
            .unwrap();
        if j["status"] == "cancelled" {
            cancelled = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(cancelled, "active loop encoder should stop");
    drop(agent);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn agent_makes_a_video_end_to_end_without_desktop() {
    let dir = std::env::temp_dir()
        .join(format!("blyacut-headless-{}", blyacut_core::id()))
        .canonicalize_or_create();
    let p = |name: &str| dir.join(name).to_string_lossy().to_string();
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=s=640x360:r=30",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=330",
        "-t",
        "3",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        &p("shot.mp4"),
    ]);
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=220:sample_rate=48000",
        "-t",
        "6",
        &p("music.wav"),
    ]);

    // Review mode keeps the human gate.
    let mut review = Agent::start(&dir, false);
    assert_eq!(
        review.ok("get_session", json!({}))["access"]["mode"],
        "review"
    );
    review.child.kill().unwrap();

    let mut a = Agent::start(&dir, true);
    assert_eq!(
        a.ok("get_session", json!({}))["access"]["mode"],
        "autonomous"
    );
    let shot = a.ok("import_media", json!({"path": p("shot.mp4")}));
    let shot_id = shot["project"]["assets"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    a.ok("import_media", json!({"path": p("music.wav")}));
    let title = a.ok("create_title_card", json!({"text":"BlyaCut\nагенты монтируют","output_path":p("title.mp4"),"duration_ms":1500,"background_color":"#1020C8"}));
    let title_id = title["asset_id"].as_str().unwrap().to_string();
    let project = a.ok("get_project", json!({}));
    let rev = project["revision"].as_u64().unwrap();
    let tracks = &project["project"]["tracks"];
    let (vt, at) = (
        tracks[0]["id"].as_str().unwrap(),
        tracks[1]["id"].as_str().unwrap(),
    );
    let music_id = project["project"]["assets"][1]["id"].as_str().unwrap();
    let presets = a.ok("list_effect_recipes", json!({}))["builtin"].clone();
    let preset = |id: &str| {
        presets
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap()
            .clone()
    };

    let stale = a.call("apply_edits", json!({"expected_revision": rev + 7, "edits":[{"operation":"delete_clip","clip_id":"x"}]})).unwrap_err();
    assert!(stale.contains(&format!("current {rev}")), "{stale}");

    let edited = a.ok("apply_edits", json!({"expected_revision": rev, "edits": [
        {"operation":"append","asset_id":title_id,"clip_id":"title","track_id":vt},
        {"operation":"append","asset_id":shot_id,"clip_id":"shot","track_id":vt},
        {"operation":"append","asset_id":shot_id,"clip_id":"extra","track_id":vt},
        {"operation":"append","asset_id":music_id,"clip_id":"music","track_id":at},
        {"operation":"set_effects","clip_id":"title","effects":[preset("builtin-fade-in")]},
        {"operation":"set_effects","clip_id":"shot","effects":[preset("builtin-vhs")]},
        {"operation":"set_audio","clip_id":"music","gain_milli":600,"fade_in_ms":300,"fade_out_ms":800},
        {"operation":"delete_clip","clip_id":"extra"}
    ]}));
    // 1.5 s title + 3 s shot = 4.5 s of video under 6 s of music.
    let warnings = edited["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("Audio runs 1.50 s past")),
        "{warnings:?}"
    );
    let rev = edited["revision"].as_u64().unwrap();
    let trimmed = a.ok("apply_edits", json!({"expected_revision": rev, "edits": [{"operation":"trim_clip","clip_id":"music","start_ms":0,"end_ms":4500}]}));
    assert!(
        trimmed["warnings"].as_array().unwrap().is_empty(),
        "{trimmed}"
    );

    let frame = a.ok(
        "preview_frame",
        json!({"time_ms": 2500, "output_path": p("frame.png"), "width": 320}),
    );
    assert!(std::path::Path::new(frame["path"].as_str().unwrap()).is_file());
    assert!(a
        .call(
            "preview_frame",
            json!({"time_ms": 0, "output_path": "/tmp/escape.png"})
        )
        .is_err());

    let rev = trimmed["revision"].as_u64().unwrap();
    a.ok(
        "export_video",
        json!({"expected_revision": rev, "output_path": p("promo.mp4")}),
    );
    let job = loop {
        let job = a.ok("get_session", json!({}))["render_job"].clone();
        if job["status"] != "rendering" {
            break job;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    assert_eq!(job["status"], "complete", "{job}");
    assert!(job["loudness"]["peak_db"].as_f64().unwrap() < 0.0);
    assert_eq!(job["output"], json!(p("promo.mp4")));
    let out = blyacut_engine::probe(std::path::Path::new(&p("promo.mp4"))).unwrap();
    assert!(out.duration_ms.abs_diff(4500) < 150, "{}", out.duration_ms);

    a.ok("save_project", json!({"path": p("promo.blyacut")}));
    let reopened = a.ok("open_project", json!({"path": p("promo.blyacut")}));
    assert_eq!(
        reopened["project"]["tracks"][0]["clips"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    drop(a.input);
    assert!(a.child.wait().unwrap().success());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn agent_layers_text_and_picture_in_picture() {
    let dir = std::env::temp_dir()
        .join(format!("blyacut-layers-{}", blyacut_core::id()))
        .canonicalize_or_create();
    let p = |name: &str| dir.join(name).to_string_lossy().to_string();
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=red:s=640x360:r=30",
        "-f",
        "lavfi",
        "-i",
        "sine",
        "-t",
        "3",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        &p("main.mp4"),
    ]);
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:s=640x360:r=30",
        "-t",
        "3",
        "-c:v",
        "libx264",
        &p("cam.mp4"),
    ]);
    let mut a = Agent::start(&dir, true);
    let main = a.ok("import_media", json!({"path": p("main.mp4")}))["project"]["assets"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let cam = a.ok("import_media", json!({"path": p("cam.mp4")}))["project"]["assets"][1]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let text = a.ok(
        "create_text_overlay",
        json!({"text":"Агент монтирует","output_path":p("caption.png"),"position":"bottom"}),
    );
    // One-call placement on a fresh overlay track, with an outline style.
    let rev0 = text["revision"].as_u64().unwrap();
    a.ok("apply_edits", json!({"expected_revision": rev0, "edits": [{"operation":"add_track","track_id":"titles","name":"Video 9","kind":"video"}]}));
    let placed = a.ok("create_text_overlay", json!({"text":"Сразу на месте","output_path":p("title.png"),"position":"top","style":"outline","place":{"track_id":"titles","position_ms":250,"duration_ms":1500}}));
    let clip_id = placed["clip_id"].as_str().unwrap().to_string();
    let titles = placed["project"]["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "titles")
        .unwrap()
        .clone();
    assert_eq!(titles["clips"][0]["id"], json!(clip_id));
    assert_eq!(titles["clips"][0]["position_ms"], 250);
    assert_eq!(titles["clips"][0]["source_end_ms"], 1500);
    let text_id_first = text["asset_id"].as_str().unwrap().to_string();
    let text = json!({"asset_id": text_id_first, "revision": placed["revision"], "project": placed["project"]});
    let text_id = text["asset_id"].as_str().unwrap().to_string();
    let rev = text["revision"].as_u64().unwrap();
    let vt = text["project"]["tracks"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let done = a.ok("apply_edits", json!({"expected_revision": rev, "edits": [
        {"operation":"add_track","track_id":"pip","name":"Video 2","kind":"video"},
        {"operation":"add_track","track_id":"captions","name":"Video 3","kind":"video"},
        {"operation":"append","asset_id":main,"clip_id":"main","track_id":vt},
        {"operation":"append","asset_id":cam,"clip_id":"cam","track_id":"pip"},
        {"operation":"trim_clip","clip_id":"cam","start_ms":0,"end_ms":2000},
        {"operation":"move_clip","clip_id":"cam","track_id":"pip","position_ms":500},
        {"operation":"set_transform","clip_id":"cam","transform":{"x_milli":800,"y_milli":200,"scale_milli":300,"opacity_milli":900}},
        {"operation":"append","asset_id":text_id,"clip_id":"caption","track_id":"captions"},
        {"operation":"trim_clip","clip_id":"caption","start_ms":0,"end_ms":3000}
    ]}));
    assert!(
        done["warnings"].as_array().unwrap().is_empty(),
        "{}",
        done["warnings"]
    );
    a.ok(
        "preview_frame",
        json!({"time_ms": 1000, "output_path": p("layers.png")}),
    );
    let rev = done["revision"].as_u64().unwrap();
    a.ok(
        "export_video",
        json!({"expected_revision": rev, "output_path": p("layers.mp4")}),
    );
    let job = loop {
        let job = a.ok("get_session", json!({}))["render_job"].clone();
        if job["status"] != "rendering" {
            break job;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    assert_eq!(job["status"], "complete", "{job}");
    drop(a.input);
    assert!(a.child.wait().unwrap().success());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn agent_retimes_and_freezes() {
    let dir = std::env::temp_dir()
        .join(format!("blyacut-speed-{}", blyacut_core::id()))
        .canonicalize_or_create();
    let p = |name: &str| dir.join(name).to_string_lossy().to_string();
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=s=320x180:r=30",
        "-f",
        "lavfi",
        "-i",
        "sine",
        "-t",
        "4",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        &p("clip.mp4"),
    ]);
    let mut a = Agent::start(&dir, true);
    let imported = a.ok("import_media", json!({"path": p("clip.mp4")}));
    let asset = imported["project"]["assets"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let vt = imported["project"]["tracks"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let rev = imported["revision"].as_u64().unwrap();
    let fast = a.ok(
        "apply_edits",
        json!({"expected_revision": rev, "edits": [
            {"operation":"append","asset_id":asset,"clip_id":"c","track_id":vt},
            {"operation":"set_speed","clip_id":"c","speed_milli":2000}
        ]}),
    );
    assert_eq!(
        fast["project"]["tracks"][0]["clips"][0]["speed_milli"],
        2000
    );
    let frozen = a.ok(
        "freeze_frame",
        json!({"clip_id":"c","at_ms":1000,"duration_ms":1500,"output_path":p("hold.png")}),
    );
    let clips = frozen["project"]["tracks"][0]["clips"].as_array().unwrap();
    assert_eq!(clips.len(), 3);
    assert_eq!(clips[1]["id"], frozen["clip_id"]);
    assert_eq!(clips[2]["position_ms"], 2500);
    drop(a.input);
    assert!(a.child.wait().unwrap().success());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn agent_transcribes_with_its_own_model_and_lays_captions() {
    let dir = std::env::temp_dir()
        .join(format!("blyacut-captions-{}", blyacut_core::id()))
        .canonicalize_or_create();
    let p = |name: &str| dir.join(name).to_string_lossy().to_string();
    std::fs::create_dir(dir.join("subs")).unwrap();
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=gray:s=320x180:r=30",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=300",
        "-t",
        "5",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        &p("talk.mp4"),
    ]);
    let mut a = Agent::start(&dir, true);
    let imported = a.ok("import_media", json!({"path": p("talk.mp4")}));
    let asset = imported["project"]["assets"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let vt = imported["project"]["tracks"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let rev = imported["revision"].as_u64().unwrap();
    a.ok("apply_edits", json!({"expected_revision": rev, "edits": [{"operation":"append","asset_id":asset,"clip_id":"talk","track_id":vt}]}));

    // 1. BlyaCut hands the agent speech audio; the agent transcribes it itself.
    let audio = a.ok(
        "extract_audio",
        json!({"clip_id":"talk","output_path":p("talk.wav")}),
    );
    assert_eq!(audio["timeline_offset_ms"], 0);
    let wav = blyacut_engine::probe(std::path::Path::new(&p("talk.wav"))).unwrap();
    assert!(wav.has_audio && wav.duration_ms.abs_diff(5000) < 100);
    let info = std::process::Command::new(blyacut_engine::binary("ffprobe"))
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=sample_rate,channels",
            "-of",
            "csv=p=0",
            &p("talk.wav"),
        ])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&info.stdout).trim(), "16000,1");

    // 2. The agent's transcript comes back as SRT and becomes legible subtitles.
    let srt = "1\n00:00:00,500 --> 00:00:01,800\nПривет, это агент\n\n2\n00:00:02,000 --> 00:00:03,000\nОн сам расшифровал речь\n\n3\n00:00:03,200 --> 00:00:04,500\nи наложил субтитры\n";
    let done = a.ok("add_captions", json!({"srt": srt, "output_dir": p("subs")}));
    let track_id = done["track_id"].as_str().unwrap();
    let track = done["project"]["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == track_id)
        .unwrap()
        .clone();
    assert_eq!(track["name"], "Subtitles");
    let clips = track["clips"].as_array().unwrap();
    let placed: Vec<(u64, u64)> = clips
        .iter()
        .map(|c| {
            (
                c["position_ms"].as_u64().unwrap(),
                c["source_end_ms"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(placed, vec![(500, 1300), (2000, 1000), (3200, 1300)]);
    assert!(
        done["warnings"].as_array().unwrap().is_empty(),
        "{}",
        done["warnings"]
    );

    // 3. Visible only while a caption is on: the plate darkens the lower frame.
    let mut lower = |name: &str, at: u64| {
        a.ok(
            "preview_frame",
            json!({"time_ms": at, "output_path": p(name), "width": 320}),
        );
        let out = std::process::Command::new(blyacut_engine::binary("ffmpeg"))
            .args([
                "-v",
                "error",
                "-i",
                &p(name),
                "-vf",
                "crop=iw:ih*0.3:0:ih*0.65,format=gray",
                "-f",
                "rawvideo",
                "pipe:1",
            ])
            .output()
            .unwrap();
        // White glyph pixels in the lower third (the gray footage has none).
        out.stdout.iter().filter(|v| **v > 220).count()
    };
    let (with, without) = (lower("on.png", 1000), lower("off.png", 1900));
    assert!(
        with > 200 && without == 0,
        "caption glyphs only while on: {with} vs {without}"
    );
    assert!(a.call("add_captions", json!({"captions":[{"start_ms":0,"end_ms":1000,"text":"a"},{"start_ms":500,"end_ms":900,"text":"b"}],"output_dir":p("subs")})).unwrap_err().contains("overlaps"));
    drop(a.input);
    assert!(a.child.wait().unwrap().success());
    std::fs::remove_dir_all(dir).unwrap();
}

trait Create {
    fn canonicalize_or_create(self) -> std::path::PathBuf;
}
impl Create for std::path::PathBuf {
    fn canonicalize_or_create(self) -> std::path::PathBuf {
        std::fs::create_dir_all(&self).unwrap();
        self.canonicalize().unwrap()
    }
}
