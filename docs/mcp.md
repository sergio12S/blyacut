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

2. In the right sidebar’s **Agent** tab, click **Enable agent access…** and choose the directory containing the footage/audio and intended exports. The app creates a private, temporary session config. Its path appears in the sidebar.
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

In the desktop app, tick **Autonomous agent** before **Enable agent access…** to give a connected agent the same authority; Undo still works.

Title cards use a system TrueType font (Arial on macOS, DejaVu/Noto on Linux; override with `BLYACUT_FONT`), because FFmpeg builds without libfreetype have no text filters.

## Available tools

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

See [effect recipes](effects.md) for schemas, parameter bounds, hot reloading and preview/export limits. There are currently 18 discoverable tools (incl. `freeze_frame`). Edits include `set_speed` (0.25×–4×, reverse), `set_transform` for overlay clips, `set_transition` (20 kinds, handles required) and `set_settings` (format, fps, crop-to-fill); `import_media` also accepts PNG/JPEG/WebP stills.
