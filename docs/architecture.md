# Architecture

The editing core is the authoritative state holder. Vue never edits a project locally; Pinia replaces its cached snapshot after a successful Rust operation.

| Layer | Location | Responsibility |
|---|---|---|
| Vue UI | `src/` | Media library, monitor, timeline, inspector, dialogs and shortcuts |
| Tauri adapter | `src-tauri/` | IPC, session mutex, file scopes, background workers and events |
| Editing core | `crates/core/` | Versioned model, validation, commands, history and JSON persistence |
| Video engine | `crates/engine/` | Probe, argument generation, isolated temporary outputs, rendering and cancellation |
| Shared live session | `crates/session/` | Revisioned snapshots, jobs, proposals, gateway permissions and private Unix IPC |
| Agent adapter | `crates/mcp/` | stdio MCP lifecycle/tool discovery, forwarding into the live session |

The core, engine, session and MCP crates have no UI/Tauri dependency. The desktop owns an `Arc<Session>`, which wraps `Editor` in a mutex. UI commands and the agent gateway call the same session methods. Revisioned events update Pinia; older snapshots are ignored. Desktop probing, saving and loading use blocking workers; exports run on a dedicated background thread. Rendering operates on an immutable snapshot; editing can continue while export runs. A single active render slot prevents competing exports. Its cancellation flag kills and reaps the FFmpeg child.

## Timeline contract

Times are unsigned integer milliseconds, bounded to one week and safe to send as JavaScript numbers. Frame rate is rational. UI stepping computes the adjacent frame from its index rather than repeatedly adding a rounded frame duration.

Schema v3 embeds validated effect recipes; v2 introduced independent audio tracks, clip linear gain and fades. The loader explicitly upgrades v1/v2 defaults. A clip holds a stable ID, asset ID, timeline position, and half-open source interval `[source_start_ms, source_end_ms)`. Tracks are ordered arrays; validation rejects duplicate IDs, overlapping/out-of-order clips, unknown assets, invalid settings and out-of-bounds source ranges. Distinct tracks can overlap; the exporter mixes independent audio tracks but rejects more than one populated video track.

Every edit first operates on a candidate project, validates it and only then commits. Failure leaves history/state untouched. Append and split carry caller-provided IDs, so replay against the same initial project is deterministic. History stores up to 100 complete project snapshots; this favors correctness at MVP scale and should eventually be replaced by bounded operation records for huge projects.

Trim uses **source** milliseconds. Split uses an absolute **timeline** position. Video delete/trim/reorder ripple that video track. Audio delete/trim preserves all other positions. Move places a clip at an explicit time on a compatible track; overlap on the same track is rejected. Batches validate atomically and commit as one Undo step. Every commit/undo/redo/open increments the live revision, and agent edits require its expected value.

## Export

1. Validate the snapshot, output extension, at most one populated video track and audio/media limits.
2. Create a unique scratch directory beside the destination.
3. Decode each video interval, normalize geometry/FPS/audio and apply validated video effects and its linked-audio filters/gain/fades. Fill video gaps and any audio-only tail with black video and silence.
4. Concat the normalized segments into a base video. For independent audio, trim/reset timestamps, apply audio recipe filters, linear gain/fades, delay to timeline position, sum with the base audio and apply a peak limiter. Copy video and encode AAC.
5. Parse FFmpeg progress throughout the stages. Publish only the completed output with a same-filesystem hard link that refuses an existing filename. Clean scratch on success/error/cancellation.

Processes use argument vectors and no shell. Media paths are canonicalized at import. FFprobe/FFmpeg input protocols are restricted to local file/pipe. Remote URLs and playlist imports are unsupported. User-selected project files are trusted local documents. The agent gateway separately canonicalizes import/output paths and restricts them to a user-selected folder.

## Security boundary and preview

The WebView has a CSP, no remote application scripts, and an initially empty asset scope. Only imported/opened asset files are allowed for preview. Dialog capabilities are limited to open/save. No network listener is opened. Opt-in agent sessions use a private 0700 directory, 0600 Unix socket/config and random per-session secret. Destructive/large agent batches require a preview proposal approved in the desktop. See MCP documentation for limits.

The monitor is a replaceable UI component. It resolves the active clip and maps playhead time to source time; HTMLVideoElement is the decoder. This is independent of FFmpeg export. A future renderer can use the same project snapshot and playhead API. Known frame/audio limitations are in README; no GPU/frame-cache/effect parity is claimed.

## Delivery phases

Phase 1 established the editing slice; Phase 2 now adds independent audio composition and a secure MCP bridge into the live editor. Natural-language interpretation remains external. Phase 3 will add model-configured whisper.cpp jobs and transcript-derived commands. Phase 4 can extend video composition/effects after preview/export parity and synchronization improve.

## Data-only effects

`core::effects` owns Recipe v1 validation; `session::effects` hot-loads bounded local JSON catalogs. Clips embed recipe copies. UI and MCP use `set_effects`; the engine compiles whitelisted numeric steps into fixed FFmpeg filters, while preview uses CSS/Web Audio. No arbitrary code, FFmpeg strings, shader or DSP plugin execution is exposed. See [effect details](effects.md).

## Pointer timeline gestures

The WebView uses Pointer Events/capture rather than HTML5 drag/drop for internal clips. `timeline-gesture.ts` computes transient geometry and command intents; it never mutates the project cache. A gesture captures its starting revision, previews insert/move/trim, and sends one desktop `edit_batch` only on pointer-up. Left-edge audio trim composes trim+move in an order that avoids intermediate neighbour collisions; one batch is one history step. Rust Session/Editor remains authoritative. Escape, pointer cancellation or an incoming project revision discards the preview. Magnetic video reordering uses `reorder_clip`; free placement uses `move_clip`; source-boundary edits use `trim_clip`. Track-kind compatibility and overlap hints are provisional UI feedback, with final validation in Rust.
