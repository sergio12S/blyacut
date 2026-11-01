# BlyaCut

A local-first, open-source desktop video editor built with Tauri 2, Rust and Vue 3. No account, subscription or cloud API is required.

**Status: 0.2 MVP — audio composition + external-agent editing + hot-loadable effects.** Import video/audio, split and trim, position independent audio tracks, set volume and fades, undo/redo, save/load and export H.264/AAC MP4. External MCP agents can operate the same live Rust session. This is an early editor, not a finished Filmora replacement.

## Download

Prebuilt unsigned macOS builds (Apple Silicon) are attached to [GitHub Releases](https://github.com/sergio12S/blyacut/releases). The app is not notarized: after unzipping, right-click **BlyaCut.app → Open** the first time, or run `xattr -dr com.apple.quarantine BlyaCut.app`. FFmpeg must still be installed (`brew install ffmpeg`).

## Run on macOS

Prerequisites: Xcode Command Line Tools, Rust 1.90+ (1.98 tested), Node.js 22+, FFmpeg with `libx264` and `aac` encoders.

```sh
brew install ffmpeg
npm ci
npm run tauri dev
```

FFmpeg/FFprobe are discovered in `/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin`, then PATH. Override executable paths with `BLYACUT_FFMPEG` and `BLYACUT_FFPROBE`. Missing tools produce a visible error; there is no silent simulated import/export.

Build a local application (unsigned development build):

```sh
npm run tauri build -- --debug --bundles app
open target/debug/bundle/macos/BlyaCut.app
```

A release executable can be built with `npm run tauri build`. Bundling is disabled by default; use `--bundles app` on macOS. Signing, notarization, installers and other platforms are not yet verified. See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for platform dependencies.

`npm run dev` serves the UI only. Editing and filesystem operations require the desktop application.

## First edit

1. Import an MP4/MOV/M4V/MKV/WebM or WAV/MP3/M4A/FLAC/OGG, or drop it into the desktop window.
2. Click **+** beside the asset: video goes to Video 1, audio-only files go to Audio 1. Add more audio tracks with the timeline toolbar.
3. Click the timeline ruler to seek. Press **S** to split at the playhead.
4. Select the unwanted section and press Delete. Following video clips ripple left. Independent audio remains anchored.
5. Drag a clip by its middle to move it; drag its left/right edge to trim. **Magnetic on** inserts/reorders video and closes gaps; **Magnetic off** places video freely, with gaps. Audio always moves freely, including between audio tracks. **Snap on** aligns either clip edge to the playhead or other clip edges; hold ⌘/Ctrl to bypass. Invalid overlaps are rejected. Video trimming ripples subsequent video; independent audio stays anchored. The inspector still provides precise source in/out, volume, fades and position.
6. Press Space to preview, or ← / → to step on the project frame grid.
7. Save a `.blyacut` project with ⌘S. Open with ⌘O. Undo/redo with ⌘Z / ⇧⌘Z.
8. Export to a **new** `.mp4` filename. Existing output files are never overwritten.

## Agent editing

Open the **Agent** tab: (1) choose the agent's folder (optionally *Autonomous*), (2) copy the ready "address" — a `claude mcp add …` command, a JSON `mcpServers` block for Claude Desktop/Cursor/Codex/Gemini, or the headless command — and (3) ask the agent for an edit. The tab shows live status ("Agent connected · N calls"). The address is a command, not a URL: MCP runs the local `blyacut-mcp` bridge over stdio and it finds the window through a private, per-session file; no network port is opened.

In the **Agent** tab, **Choose folder and connect…** authorizes one media/export folder and creates a private session config. Build `cargo build -p blyacut-mcp` once; the tab finds the bridge and shows the exact command or JSON to paste into your MCP host. Agent edits immediately appear in the desktop, share Undo/Redo, and use revision-checked transactions. Removals/large batches have a preview-and-apply flow unless you tick **Autonomous**.

Agents can also work with no app open: `blyacut-mcp --headless FOLDER --autonomous` is a complete local studio (import, title cards, edits, effects, frame previews, export with loudness report, save/open). See [MCP setup, headless studio and reproducible demo](docs/mcp.md).

For music videos, MCP can make bass-reactive visualizers, compare short style previews, suggest energetic excerpts, produce thumbnail variants, and queue a full video plus three vertical Shorts. It can also build long, crossfaded loop mixes from chosen ranges. These create standalone files rather than timeline edits; excerpt selection is heuristic, and subtitle or word timings must be reviewed. See the [music studio workflow](docs/mcp.md#music-studio-workflow) and [loop mixes](docs/mcp.md#long-music-mixes-with-a-loopable-boundary).

BlyaCut embeds no AI model by design: the connected agent brings its own. For subtitles the agent calls `extract_audio` (16 kHz mono WAV of a clip), transcribes it with its own speech-to-text, and sends the timed text back with `add_captions` (list or SRT), which lays legible captions on a Subtitles track. People can import an `.srt` file from the Titles tab. Pause detection and generative video/music are not implemented.

## Effect recipes and interface design

Load JSON effect chains from a folder in the clip inspector, refresh while running and apply through the same Rust commands used by agents. 43 validated steps (color, blur, grain, pixelate, VHS shift, trails, fades; EQ, reverb, pitch, robot, bitcrush, compressor, chorus and more) and 36 built-in presets are available without loading anything. Applied recipes are embedded in the saved project. New combinations need no reinstall; new shader/DSP algorithms are not implemented. See [effect contract and preview limits](docs/effects.md).

The GBRAIN design swarm produced [UX prototypes and the consolidated design decisions](docs/design/decision.md). Inspector and Agent are now separate tabs; agent proposals have readable edit descriptions.

## Persistence and recovery

Project schema v4 explicitly migrates v1–v3 on load and rejects unknown versions. The versioned JSON project stores references, not media copies. Media inside the project directory uses relative paths; external files retain absolute paths. Copy referenced media alongside the project for portability. Opening a project lists missing media; relinking UI is not implemented.

While edits are unsaved, a recovery snapshot is written every 30 seconds to the OS application data directory. On macOS: `~/Library/Application Support/org.blyacut.editor/autosave.blyacut`. Open that file through **Open** to recover. There is one recovery slot; a later session can replace it. Manually save before opening another project or closing the app; there is no unsaved-change prompt yet.

## Verification

```sh
npm run build
npm run test:timeline
cargo test -p blyacut-core -p blyacut-engine -p blyacut-session -p blyacut-mcp
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Integration tests require real FFmpeg/FFprobe. They synthesize fixtures; verify cuts, persistence, audio delays/gain/fades and mixing across tracks; probe output dimensions/duration and black video gaps; check cancellation/no-overwrite, scoped paths, IPC auth, MCP lifecycle, human proposals and a real stdio subprocess. No model is used.

## Current limits

- The first video track is the magnetic main track (with linked source audio). Further video tracks are overlay layers: up to 32 overlay clips with position, scale and opacity (picture-in-picture, logos, text over video, still images); overlay clips export without sound. Up to 64 independent audio clips. Main-track gaps and tails export as black frames. 20 transitions between main-track clips (crossfade, dip to black/white, wipes, slides, circle, pixelize, zoom…) are centred on the cut and use source handles, so the timeline length never changes. Format presets include 1080p/720p/4K, 60 fps, vertical 9:16 and square with crop-to-fill. Clips can be retimed 0.25×–4× (pitch preserved) or reversed, and a freeze frame holds the frame under the playhead. Overlay clips animate position, scale and opacity with keyframes (slide-in, fade-in and zoom presets). Music on audio tracks can duck under voice by a chosen dB. No rotation yet; transitions do not yet join retimed clips; the monitor does not play ducking.
- HTML5 video/audio preview depends on WebView codec support. H.264 MP4, WAV or AAC are the most predictable. Importable containers such as MKV may export correctly but cannot preview in WebKit.
- Preview uses a wall clock and `requestAnimationFrame` to seek/synchronize independent media elements and apply linear volume/fades. Playback under buffering and across cuts is approximate; no sample-accurate AV synchronization or decoded-frame stepping is guaranteed. Final mixing sums audio with a peak limiter; preview does not model that limiter, so loud overlapping tracks can sound different. Decoder and encoder resampling also differ.
- Output uses the project format (default 1080p30; presets in the export dialog), letterboxing or crop-to-fill, CPU `libx264` CRF 20 and AAC stereo. No hardware encoding, proxies, frame cache, multiband EQ or automatic ducking yet. The monitor shows transitions as a hard cut; export and `preview_frame` outside the transition are exact.
- Clips normalize independently before concatenation. Non-frame-aligned cuts round to output frames; many cuts can accumulate duration differences. Temporary render space includes normalized segments, a base video and final mix. Cancellation cleans scratch output.
- MCP currently uses macOS/Linux Unix IPC and pinned 2025-11-25 / 2025-06-18 stdio protocols. It requires opt-in access and a separately built bridge binary. Other MCP protocol revisions/hosts and Windows named pipes are not verified.
- Built-in speech recognition (by design, agents transcribe), silence removal, generative media providers, SQLite metadata indexing and public-release packaging are not implemented.

[Architecture](docs/architecture.md) · [MCP integration](docs/mcp.md) · [Licenses and distribution](docs/licensing.md) · [Contributing](CONTRIBUTING.md)
