# BlyaCut vs Filmora: gap analysis and roadmap

Clean-room comparison from **public** Wondershare documentation and independent reviews only. No Filmora binaries, code, assets or project files were inspected. Feature names describe user-visible behavior, not implementation. Sources at the end. Snapshot: 2026-10-19, BlyaCut `main` @ 125b454.

## Interface model

| Area | Filmora (public guides) | BlyaCut now | Gap |
|---|---|---|---|
| Library | Top tabs: Media, Audio, Titles, Transitions, Effects, Elements/Stickers, Templates; drag onto the timeline | Media list only; effects live inside the inspector | **Library tabs** with drag-and-drop (Media · Titles · Effects; Transitions later) |
| Player | Preview with editable timecode, snapshot, mark in/out, playback quality | Preview, frame step, timecode read-only | Editable timecode, snapshot, quality toggle |
| Property panel | Tabs per selected clip: Video, Audio, Color, Speed | Single scrolling column, source times in **milliseconds**, explicit *Apply trim* button | **Tabbed inspector**, timecode fields, apply on Enter |
| Toolbar | Cut, crop, speed and more above the timeline | Split, delete, audio track, magnetic/snap | Merge undo/redo and tools into one bar |
| Chrome | Compact single header | Header + history bar + timeline toolbar + hint row + status bar | Remove two rows; monitor gets the space |

## Features

| Capability | Filmora | BlyaCut | Priority |
|---|---|---|---|
| Trim/split/reorder, multiple audio tracks | ✔ | ✔ | — |
| Effects & filters | Large library | 43 steps, 36 presets, custom JSON recipes | — |
| Agent / automation editing | Not a focus | ✔ MCP, headless autonomous studio | BlyaCut's edge |
| Titles | 200+ animated templates, text editor | Still title cards (agents only) | **P1**: title tab in UI, then text overlay |
| Multiple video tracks / PiP / overlays | ✔ | ✔ overlay tracks with position/scale/opacity | — |
| Transitions | 100+ in categories | ✔ 20 kinds, handle-based, length-preserving | more kinds, monitor preview |
| Speed, reverse, freeze frame, speed curves | ✔ | ✔ 0.25×–4×, reverse, freeze frame | speed curves |
| Keyframes (position/scale/opacity/volume) | ✔ | ✔ overlays: position/scale/opacity + presets | volume keyframes, easing |
| Crop/scale/position | ✔ | ✔ for overlays; crop-to-fill for the frame | per-clip crop |
| Auto subtitles (speech-to-text) | ✔ | Planned (whisper.cpp) | P2 |
| Audio ducking | ✔ | ✔ sidechain, chosen depth | — |
| Chroma key | ✔ | — | P3 |
| Motion / camera tracking, AI generation | ✔ | — | P3 / out of scope locally |
| Export presets (resolution, vertical, social) | ✔ | ✔ 1080p/720p/4K, 60 fps, 9:16, 1:1 | bitrate/codec choice |
| Thumbnails/waveforms on timeline | ✔ | Decorative stripes | P2 |

## Roadmap

**Phase 1 — interface (now).** Library tabs (Media · Titles · Effects) with drag onto clips; tabbed inspector (Clip · Audio · Effects) with timecode fields; one compact header with undo/redo and export progress; no hint row; consistent Russian UI; title cards from the UI.

**Phase 2 — composition (done: overlay tracks, PiP, stills, legible text over video, 20 transitions, format presets incl. vertical crop-to-fill).** Multiple video tracks with overlay/PiP (position, scale, opacity), text overlay rendered as an image layer, transitions between adjacent clips, export presets. Preview parity first: the monitor must show what export renders.

**Phase 3 — motion and time (started: speed, reverse, freeze frame).** Speed/reverse/freeze, keyframes, ducking, whisper.cpp subtitles, timeline thumbnails and waveforms.

Each phase keeps the rule that UI and agents share one Rust edit model: every new capability is an `Edit` operation and an MCP tool at the same time.

## Sources
- Wondershare, *Navigating Filmora*: https://filmora.wondershare.com/guide/filmora-navigating-filmora.html
- Wondershare, *Panel layout* (property panel, player): https://filmora.wondershare.de/guide/panel-layout.html
- Wondershare, *Features*: https://filmora.wondershare.com/filmora-features.html
- Wondershare, *What's new*: https://filmora.wondershare.com/whats-new-in-filmora-video-editor.html
- Wondershare, *Filmora for Mac*: https://filmora.wondershare.com/video-editor-mac/
- Movavi, *Wondershare Filmora review* (competitor review): https://www.movavi.com/adv/wondershare-filmora-review.html
