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

trait Create {
    fn canonicalize_or_create(self) -> std::path::PathBuf;
}
impl Create for std::path::PathBuf {
    fn canonicalize_or_create(self) -> std::path::PathBuf {
        std::fs::create_dir_all(&self).unwrap();
        self.canonicalize().unwrap()
    }
}
