# Local verification — 2026-10-18

Environment: macOS / Apple Silicon, Rust 1.98.0, Node 22.12.0, Tauri 2.12.4, FFmpeg/FFprobe 9.0.2 installed through Homebrew.

Passed:

- `npm run build`: strict Vue/TypeScript check and Vite production bundle.
- `cargo test -p blyacut-core -p blyacut-engine`: 10 integration/unit contract tests (8 core, 2 engine). The engine test performs real encode/probe for both audio and silent sources, cut/delete, persistence, cancellation, cleanup and refusal to overwrite.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo fmt --all --check`.
- `npm run tauri build -- --debug --bundles app`: unsigned macOS application at `target/debug/bundle/macos/BlyaCut.app`.
- `python3 scripts/generate-example.py` and the Rust export example: `cargo run -p blyacut-engine --example export -- examples/first-cut.blyacut /tmp/BlyaCut-first-cut.mp4`.
- FFprobe on that example: H.264 video, AAC audio, 640×360, 4.021333 seconds for a 4-second edited timeline (AAC/container timing tolerance).

## Native desktop acceptance — passed after permissions became available

The first attempt was denied by Computer Use permissions. After the user granted access, the bundled `.app` launched successfully and its native UI was exercised:

1. Import `examples/synthetic.mp4` through the macOS file dialog; media library reports 6 seconds, 640×360, audio.
2. Append to the timeline and display a decoded frame in the WebView monitor.
3. Advance 60 project frames to 2.000 seconds, split, advance 60 more to 4.000 seconds, split again.
4. Select and delete the middle clip. The timeline becomes two clips totaling 4 seconds.
5. Start playback and verify playhead advancement with a changing decoded image. Seek to timeline 2.000 seconds: the visible source timecode is 4.000 seconds, confirming removal of the middle section in preview.
6. Undo restores three clips / 6 seconds; redo restores two clips / 4 seconds.
7. Save as `examples/gui-acceptance.blyacut`, then open that file through the native dialog. Both edited source ranges survive; the asset path is portable (`synthetic.mp4`).
8. Export through the UI to `examples/gui-acceptance-export.mp4`. Progress was visible at 48%, followed by **Export complete**.
9. FFprobe confirms H.264 video at 1920×1080 / 30 fps, AAC audio, duration 4.021333 seconds.
10. The UI reported a recovery snapshot; the autosave file was confirmed on disk in the application data directory.

The saved GUI acceptance project and exported MP4 remain in `examples/` for inspection. The original synthetic fixture is separate from the output.

## Phase 1 verification limits (historical)

This is a narrow acceptance run on synthetic H.264/AAC media. Native file drag/drop, drag reordering, trim through the inspector, restoring from autosave, UI-triggered in-flight cancellation, sample-accurate audio synchronization and frame-accurate playback have not been verified in this run. Cancellation is covered by the Rust integration test. CI workflows were authored but have not run on hosted runners. Windows/Linux desktop builds and public release signing/licensing clearance are outstanding. At that milestone MCP, transcription and multitrack composition were unimplemented; the 0.2 section below supersedes the MCP/audio status.


## 0.2 — independent audio and live MCP editing

Passed 20 Rust tests (11 core, 3 engine, 4 session, 2 MCP), strict workspace clippy, formatting, production frontend build and macOS debug bundling. The engine test decodes PCM samples and checks delayed onset, silence outside the clip, volume ratios, fade-in/out, summing of overlapping tracks and an actual black frame in a video gap.

The official Python MCP SDK **1.30.0** connected to the bundled desktop through `blyacut-mcp`. Its scripted client initialized, discovered 10 tools, imported two real synthetic MP4 files plus a WAV test tone, then applied nine operations as one batch. The native UI immediately showed two 2-second video clips at 20% gain and one 4-second audio clip at 25% gain / 500 ms fades. Export was started and polled through MCP. `examples/agent-audio-export.mp4` is H.264/AAC at 1920×1080, duration 4.021333 seconds; full FFmpeg decode passed.

The live removal proposal was confirmed to leave revision/timeline unchanged before approval. Its exact command appeared in the desktop review panel. Clicking Apply removed the audio clip; a single desktop Undo restored it. UI volume adjustment (25→30%), audio positioning (0→500 ms) and trimming (4→3.5 seconds) were also exercised, then undone. The original test mix was saved through the native dialog as `examples/agent-audio-demo.blyacut` (schema v2, relative asset paths). Playback advanced in the WebView without an error; auditory synchronization/limiter parity was not measured. Stop agent access removed the session config/socket. Unit/integration tests separately verify auth, malformed arguments, symlink escape rejection, stale revisions, proposal invalidation, batch rollback, v1 migration and actual stdio subprocess control.

After this run the user made further manual edits in the open app. That session was left running without a restart to preserve those unsaved changes. The final 0.2 build is prepared on disk; the running process may still show the earlier 0.1 badge used during development.

Remaining: no built-in natural-language model, whisper.cpp, subtitles, silence removal, automatic ducking, generative media or layered video compositor. No claim of sample/frame-accurate preview, compatibility with all MCP hosts/protocol revisions, Windows IPC, hardened same-user process isolation or hosted CI success. The Python demo is a scripted protocol client, not an autonomous language-model evaluation.

## Recipe effects and first design-swarm integration

`cargo test --workspace`: **23 passed** (12 core, 4 engine, 5 session, 2 MCP). New coverage verifies strict recipe validation, atomic effect changes and history, embedded project recipe snapshots, dynamic catalog refresh/create/path restrictions, real grayscale pixels in encoded video, low-pass attenuation and the measured delayed copy of an impulse (echo) without extending clip duration. Workspace clippy with warnings denied and production Vue/TypeScript build passed.

A separately identified **BlyaCut QA** app (`org.blyacut.qa`) was built and launched without modifying the user's original editor session. The official Python MCP SDK 1.30.0 discovered all 12 tools, composed a three-clip test timeline and exported it. In the same running QA session, `create_effect_recipe` wrote a new monochrome JSON, `list_effect_recipes` immediately returned it, and a three-operation `set_effects` batch applied it to both video clips plus a low-pass recipe to the WAV clip. No app restart occurred between recipe creation and application. Agent export completed; `examples/qa-effects-export.mp4` probes as H.264/AAC, 1920×1080, duration 4.021333 seconds and fully decodes without errors.

Native QA verified:

- The program monitor displays a grayscale source frame after the effect edit.
- Inspector and Agent tabs switch independently; the proposal counter updates live.
- An agent proposal appears as readable text: soundtrack volume 30%, fade in/out 500 ms; technical JSON is collapsed. Clicking Apply updates the timeline's audio gain badge.
- A schema-v3 project with embedded effects was saved through the native dialog as `examples/qa-effects-demo.blyacut`, then opened in the updated QA app. Three clips and their recipes survive.
- Source In/Out are side by side, audio settings are a collapsed section, and the effect chain remains in the inspector's scrollable body.
- Clicking Agent and pressing Left returns focus/selection to Inspector without moving the playhead.
- Test MCP access was stopped; its config/socket disappeared.

The final native UI pass was at the default window size. Minimum-size layout, mouse trim, arbitrary effect combinations, auditory preview quality and preview/export parity are not acceptance-tested. One monochrome frame is evidence of color-preview wiring, not a general effect-parity certification. DSP export behavior is measured by Rust integration tests; native Web Audio quality is not measured.

Claude and AGY supplied standalone design prototypes and briefs through GBRAIN. Artifact structure checks pass for both roles. Grok produced no deliverable because its API rejected output-token reservation (HTTP 402); a deprecated fast variant also failed (404). The [design synthesis](design/decision.md) distinguishes working implementation from proposed UI/API features. Prototype screenshots reported by a worker are not counted as main-agent browser acceptance; the live Vue/Tauri QA above was directly observed.


## Pointer timeline editing — 2026-10-18

A new `BlyaCut Drag QA` bundle (`org.blyacut.dragqa`) kept the user's open `BlyaCut QA` session untouched. The existing saved three-clip example was opened for temporary tests without overwriting it.

Native mouse gestures verified:

- Move the second video before the first with Magnetic on: both source ranges survive, positions swap 0/2000 ms; one Undo restores the order.
- Turn Magnetic off and drag the second video to 5000 ms: the first stays at zero and project duration becomes 7000 ms.
- Move audio to 1000 ms, trim its right edge from 4000 to 3000 source ms, then trim the left edge to 500 ms: timeline position becomes 500 ms and duration 2500 ms, preserving its right edge. One Undo restores the left-trim gesture's two edits together.
- Add Audio 2 and drag audio onto it at 1000 ms; Video 1 and Audio 1 remain unchanged.
- Drop video over an occupied region in free mode: a clear overlap message appears and clip positions remain unchanged.
- Trim the first video from 2000 to 1500 ms by its right edge: the following video moves to 1500 ms; audio remains anchored.
- With Snap on, drop the video near the audio end: its position resolves to exactly 4000 ms.

During QA, the literal clip border initially initiated a move instead of edge trimming. Hit testing now includes the border in a 10 CSS-pixel edge region, preserving a central move target even on narrow clips.

`npm run test:timeline` passes 14 checks using Node assertions and the existing TypeScript compiler, including source bounds, unequal-length reorder, audio trim ordering, frame-grid rounding, both-edge snap, overlap/track-kind rejection, border hit testing and immutable project snapshots. The new Rust session test executes both audio left-trim command orders next to an adjacent clip and confirms one-revision/one-Undo behavior. Existing export/MCP tests remain relevant; editor commands now enter the same Session through desktop `edit_batch`.

Limitations: single-clip gestures only; no overwrite/automatic track creation on overlap, multiselect, linked-clip group movement or custom track heights. Video trimming still ripples even with Magnetic off, as the footer explicitly states. Autoscroll, Escape/pointer-cancel handling, Ctrl/Cmd snap bypass and incoming-revision cancellation are implemented but not exercised by the native drag acceptance run. Cross-platform pointer behavior and minimum-window layout remain unverified.

Final pass: 24 workspace Rust tests and 14 gesture checks pass. Native QA after the border-hit correction confirmed dragging the exact outer edge changes the source out-point (4000→3000 ms) rather than moving the clip. A full-suite run also exposed an intermittent local-IPC Broken pipe; accepted streams now explicitly use blocking I/O with the existing bounded timeouts. The IPC test issues 32 consecutive authenticated requests before the existing bad-auth/revocation checks; the full suite passes with this change.
