# MCP agent integration

BlyaCut 0.2 exposes a working **stdio MCP bridge into the running desktop session** on macOS/Linux. No LLM account, API key or network listener is required by the editor. An external MCP host supplies the agent/model; the application itself does not yet interpret natural-language prompts or generate media.

## Connect

1. Build/run the desktop app and build the bridge:

   ```sh
   npm ci
   npm run tauri dev
   # In another terminal:
   cargo build -p blyacut-mcp
   ```

2. In the right dock open **Agent** and click **Choose folder and connect…**, choosing the directory with the footage/audio and intended exports. The app creates a private, temporary session config and shows copy-ready connection snippets (a `claude mcp add` command, a JSON `mcpServers` block, the headless command) plus live call status.
3. Configure your MCP host to launch `target/debug/blyacut-mcp`, passing that exact config path as its first argument. A generic host configuration example:

   ```json
   {
     "mcpServers": {
       "blyacut": {
         "command": "/absolute/path/to/BlyaCut/target/debug/blyacut-mcp",
         "args": ["/tmp/blyacut-SESSION-ID/session.json"]
       }
     }
   }
   ```

   Configuration syntax varies by host. The executable also accepts `BLYACUT_SESSION` when no argument is supplied. Session paths rotate when access is enabled again; there is no persistent credential. The bridge binary is currently a separate source-build artifact, not bundled into the `.app`.
4. Ask the agent to inspect the project, import media, apply a batch and export. Changes immediately appear in the desktop timeline.
5. **Stop agent access** revokes that endpoint and deletes its config/socket. A render already started continues until complete or cancelled separately.

## Headless autonomous studio

Agents can make videos with no desktop app and no human in the loop:

```sh
cargo build -p blyacut-mcp
target/debug/blyacut-mcp --headless /path/to/folder --autonomous
```

The process owns an in-process session limited to that folder. `--autonomous` lets `apply_edits` include `delete_clip` and up to 100 edits; `propose_edits` is then refused because nobody reviews. Without the flag the review rules below apply (and nothing can approve proposals headlessly). On stdin EOF the bridge waits for a running export to finish.

Recommended loop: `import_media` / `create_title_card` → `get_project` → `apply_edits` → read `warnings` (black gaps, audio running past video) → `preview_frame` and inspect the PNGs → `export_video` → poll `get_session` until `render_job.status` is `complete`, then read `render_job.loudness` (mean/peak dBFS) and `render_job.warnings` (e.g. limiter clipping) → `save_project`.

Agents without their own MCP client can run one persistent session from a Python script that has `call(tool, args)` in scope:

```sh
uv run --with 'mcp<2' python scripts/mcp-script.py --headless /path/to/folder --autonomous plan.py
```

In the desktop app, tick **Autonomous** before **Choose folder and connect…** to give a connected agent the same authority; Undo still works.

Title cards use a system TrueType font (Arial on macOS, DejaVu/Noto on Linux; override with `BLYACUT_FONT`), because FFmpeg builds without libfreetype have no text filters.

## Subtitles: the agent transcribes, BlyaCut lays them out

BlyaCut embeds no speech model. An agent with its own speech-to-text:

1. `extract_audio {clip_id, output_path: ".../talk.wav"}` → 16 kHz mono WAV; `timeline_offset_ms` says where WAV time 0 sits on the timeline.
2. Transcribe it with your own model; add `timeline_offset_ms` to the timings.
3. `add_captions {srt: "...", output_dir: ".../subs"}` or `{captions: [{start_ms, end_ms, text}], ...}` → legible captions (box plate by default, vertical-safe positions, automatic wrapping) on a new Subtitles overlay track in one step.

## Available tools

For a finished music video, `render_bass_visualizer` analyzes low-frequency audio energy and writes a new MP4 with bass-driven zoom and shake, without changing the timeline. Example arguments: `{"input_path":"/authorized/song.mp4","output_path":"/authorized/song-visual.mp4","intensity":65}`. Intensity ranges from 1 to 100 (default 55). The synchronous call can take several minutes; videos are limited to 10 minutes. It stream-copies the original audio and subtitles, so their codecs must be MP4-compatible. The effect crops the edges and does not detect musical beats or analyze a separate audio track. A failed export may leave a partial output file.

`render_dirty_phonk` adds transient accents, green or red flashes, RGB channel shift, animated grain and scanlines, and a waveform from the input audio. Example: `{"input_path":"/authorized/song.mp4","output_path":"/authorized/phonk.mp4","intensity":65,"options":{"flash_color":"green","drop_ms":24000}}`. Each effect has a boolean option: `flash`, `glitch`, `grain`, `waveform`, `drop` (all default true). `drop_ms` explicitly sets the drop; absent it, the tool estimates a rise in bass after the intro, and may return no drop if no rise is found. The response reports `drop_ms`, `drop_estimated`, and `detected_accents`; these are energy heuristics, not guaranteed musical beat/drop detection. A drop darkens the preceding 700ms and adds a short hit. The same time, codec, scope, crop and standalone-output restrictions apply. Run/build the updated desktop backend as well as the bridge when using a desktop session.

`render_release_package` takes a finished video (for example, the output of `render_dirty_phonk`) and writes a 1920×1080 full video plus three 720×1280 vertical Shorts into an existing authorized folder. Example: `{"input_path":"/authorized/phonk.mp4","output_dir":"/authorized","prefix":"song","title":"Song title","short_seconds":15}`. The default 15-second excerpts are selected from energetic audio windows, one per third; pass `starts_ms` with three explicit timeline offsets to choose your own. Embedded subtitles, when present, are burned into the Shorts at their original timings; otherwise the title appears. Review generated excerpts and caption accuracy/timing before publication. Full audio is copied, Shorts audio is encoded to AAC. This synchronous call can take minutes and may leave partial outputs if a later render fails. It does not edit the timeline and never overwrites existing output names.

### Music studio workflow

1. `analyze_music_candidates {input_path, seconds:15}` proposes three non-overlapping windows, scored respectively for bass energy, a rise in energy, and frequency of bass onsets. These are signal heuristics, not recognition of choruses, words or actual beat grids. Listen before selecting.
2. `render_music_previews {input_path, output_dir, prefix, start_ms, seconds:10, intensity:55}` synchronously exports three short comparisons (`dirty`, `neon`, `vhs`), capped at 720px. `render_dirty_phonk` also accepts `options.look: "warehouse"`. The styles use color, grain and existing reactive effects; there is no generated footage.
3. `create_music_thumbnails {input_path, output_dir, prefix, title, times_ms:[...three times...]}` renders three 1280×720 frame-and-text variants plus editable title/description templates. These are simple compositions; review them before use.
4. `start_release_jobs {jobs:[{input_path,output_dir,prefix,title,short_seconds,starts_ms,captions_srt,caption_mode,look,intensity}]}` accepts 1–10 video tracks (up to 16 queued/running per gateway). Tracks run sequentially. Optional `look` (`dirty`, `neon`, `vhs`, or `warehouse`) first creates a bass-reactive `prefix-styled.mp4`; omit it for input already styled. Supply user-checked, non-overlapping SRT with at most two lines per cue to replace embedded subtitles in the Shorts; the full video retains its original subtitle stream. Set `caption_mode:"none"` to omit subtitle streams and text overlays entirely (already-burned text remains). `starts_ms` uses the candidate times, or omit it for energy-only selection.
5. Poll `get_music_jobs`, then retrieve each job by ID in the returned list. `completed_steps` counts finished outputs out of four, or five if a look was applied; it is not a percentage within the current output. `cancel_music_job {job_id}` cancels queued work or terminates its current FFmpeg export. Cancellation or failure can leave completed and partial files, and names are never overwritten. Jobs/history are in-memory for this gateway; keep the headless MCP process or desktop gateway alive. On headless stdin EOF the server waits for active music jobs before exiting.

SRT timings must be supplied by a human or an external transcription/alignment model. For simple word-by-word karaoke in Shorts, pass `caption_mode:"word"` and a `captions_srt` with exactly one word and its real timing per cue. The active word appears in green by itself; this does not highlight a word within a full lyric line. Maximum 40 cues per Short and 500 per SRT. The system does not infer word timings or verify lyrics. Preview and thumbnail calls are synchronous; use short preview ranges. A desktop MCP bridge process and the desktop backend both need rebuilding/restarting after changes.

For an album, `list_music_inputs {folder}` lists up to 100 video files in an authorized folder (non-recursive). Use the returned paths to populate `jobs` in batches of at most 10. The current pipeline expects video with audio; audio-only files need to be combined with cover art first.

### Long music mixes with a loopable boundary

`start_loop_mix` queues one long video from 1–8 selected ranges of video sources with audio. Example: `{"segments":[{"path":"/authorized/song.mp4","start_ms":37000,"end_ms":46000},{"path":"/authorized/song.mp4","start_ms":98000,"end_ms":107000}],"output_path":"/authorized/mix.mp4","duration_seconds":600,"fade_ms":1000}`. The tool normalizes video to 30 fps and a default 1280×720 canvas, crossfades both picture and audio at each join, creates a repeatable cycle, and crossfades the output's end back into its beginning. A 5 ms audio edge ramp reduces AAC clicks at the final loop boundary. The output is exactly the requested duration (10–3600 seconds); even `width` and `height` can be supplied for a different format. Each selected range must be no longer than 60 seconds and longer than twice the fade (300–5000 ms).

Poll `get_music_jobs` using the returned job ID, or `cancel_music_job` to stop it. Progress has two stages, not intra-stage percent. No subtitle streams are mapped into the output, but text permanently burned into input video cannot be removed; use uncaptioned source videos and select their time ranges. Fades do not detect beats or harmonically match tempos/keys, so audition the joins and loop boundary. There is no automatic creation of new music. Jobs remain session-scoped, and cancellation may leave a partial output.

| Tool | Behavior |
|---|---|
| `get_project` | Live `{project, revision}` snapshot |
| `get_timeline` | Tracks, duration and revision |
| `get_session` | Authorized folder, latest render job, pending proposal |
| `import_media` | Probe/import video or audio inside the selected folder |
| `apply_edits` | Atomic batch, one Undo, up to 20 non-removal edits |
| `propose_edits` | Validate/preview up to 100 edits; user applies/rejects in desktop |
| `undo`, `redo` | Shared history, guarded by expected revision |
| `export_video` | Start an asynchronous snapshot export to a new MP4 filename |
| `cancel_export` | Request cancellation of the current export |
| `preview_frame` | Render one timeline instant (effects included) to a new PNG in the folder |
| `render_bass_visualizer` | Render a standalone finished video with bass-driven zoom and shake |
| `render_dirty_phonk` | Render a standalone music video with selectable reactive phonk effects |
| `render_release_package` | Produce a landscape full video and three captioned vertical Shorts |
| `analyze_music_candidates` | Score three non-overlapping possible excerpts using bass-energy heuristics |
| `list_music_inputs` | Find video inputs within one authorized folder for batch processing |
| `render_music_previews` | Produce three short visual style comparisons |
| `create_music_thumbnails` | Produce three frame-and-title image variants and text templates |
| `start_release_jobs`, `get_music_jobs`, `cancel_music_job` | Queue, monitor and cancel sequential background release exports |
| `start_loop_mix` | Queue a caption-free, crossfaded long mix from selected video ranges |
| `create_title_card` | Rasterize Latin/Cyrillic text into a new MP4 title clip and import it |
| `create_text_overlay` | Transparent full-frame PNG with text (`style`: box plate by default, outline, shadow), imported as a still; `place` puts it on an overlay track at a time and duration in the same call |
| `save_project` / `open_project` | Persist or load a `.blyacut` inside the folder (its media must be inside too) |
| `list_effect_recipes` | Reload validated recipes from `effects/` in the authorized folder |
| `create_effect_recipe` | Validate and create a new recipe file without overwriting |

Every timeline mutation uses `Editor::batch`, including manual edits. The adapter does not implement an alternative editing model. It does not expose raw project-file writes, shell execution, deletion of source files, downloads or arbitrary filesystem reads.

### Batch example

First read `get_project`; use real asset/track IDs and its latest revision. Generate stable unique IDs for new tracks/clips.

```json
{
  "expected_revision": 7,
  "edits": [
    { "operation": "append", "asset_id": "imported-song-id", "track_id": "audio-track-id", "clip_id": "new-song-clip-id" },
    { "operation": "trim_clip", "clip_id": "new-song-clip-id", "start_ms": 0, "end_ms": 4000 },
    { "operation": "move_clip", "clip_id": "new-song-clip-id", "track_id": "audio-track-id", "position_ms": 500 },
    { "operation": "set_audio", "clip_id": "new-song-clip-id", "gain_milli": 250, "fade_in_ms": 500, "fade_out_ms": 500 }
  ]
}
```

Supported operations: `add_track`, `append`, `split_clip`, `trim_clip`, `move_clip`, `reorder_clip`, `set_audio`, `set_effects`, `delete_clip`. Tool discovery provides their JSON Schemas.

Times are unsigned integer milliseconds. Split/move use timeline coordinates; trim uses source coordinates. Gain is linear `0..1000` (1000 = unity), fades are relative to clip boundaries and their sum cannot exceed clip duration. Splitting inside an active fade region is rejected; clear or adjust the fade first. Audio trim/delete preserves other audio positions. Video trim/delete/reorder ripples that video track; audio stays anchored. Overlap on one track is rejected; use separate audio tracks to mix overlapping clips.

Any failure leaves the batch and history unchanged. A stale `expected_revision` fails: read again and re-plan instead of blindly retrying. Undo/redo also increments revision. Imports are guarded against project changes during probing. Saving from the UI does not clear the dirty indicator for newer edits that arrived while saving.

### Review and permissions

Removing clips or applying more than 20 operations requires `propose_edits`. This computes a candidate project without changing the timeline. The sidebar shows the concrete commands and result duration; only the desktop can apply/reject it. Approval is invalidated by intervening edits. The agent can poll `get_session` / `get_project` to observe the decision.

The local endpoint uses a random per-session secret in a 0600 config file, a 0600 Unix socket and a 0700 temporary directory. Import paths and output parent directories are canonicalized and checked against the folder selected in the UI; symlink escapes are rejected. The new output must be an absolute `.mp4` path and must not exist. This is a local agent boundary, not isolation from another process running as the same OS user. New Windows named-pipe transport is pending; manual editing architecture remains platform-independent.

## Reproducible client demonstration

Generate test media (a moving test pattern, blue video and an audio tone), then enable agent access to `examples/` in an empty desktop timeline:

```sh
python3 scripts/generate-example.py
cargo build -p blyacut-mcp
uv run --with 'mcp<2' python scripts/mcp-smoke.py \
  --session /tmp/blyacut-SESSION-ID/session.json \
  --video examples/synthetic.mp4 \
  --video examples/blue.mp4 \
  --audio examples/soundtrack.wav \
  --output examples/new-agent-export.mp4
```

This scripted client uses the official Python MCP SDK, not a built-in LLM: it initializes, discovers tools, imports three assets, applies nine edits as one action, starts export and polls its job. It refuses a nonempty timeline. `scripts/mcp-call.py SESSION TOOL 'JSON_ARGUMENTS'` can invoke individual tools. The desktop and bridge are Rust-only at runtime; Python/uv and the SDK are optional test/demo dependencies.

The live desktop path was tested with official SDK **1.30.0** and its 2025-11-25 protocol lifecycle. The bridge also negotiates 2025-06-18. It does not implement the newer 2026 protocol revision or claim compatibility with every MCP host. Sources: [MCP stdio transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports), [lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle), [official Python SDK](https://github.com/modelcontextprotocol/python-sdk).

See [effect recipes](effects.md) for schemas, parameter bounds, hot reloading and preview/export limits. There are currently 20 discoverable tools (incl. `freeze_frame`, `extract_audio`, `add_captions`). Edits include `set_keyframes` (overlay animation), `set_ducking` (music under voice), `set_speed` (0.25×–4×, reverse), `set_transform` for overlay clips, `set_transition` (20 kinds, handles required) and `set_settings` (format, fps, crop-to-fill); `import_media` also accepts PNG/JPEG/WebP stills.
