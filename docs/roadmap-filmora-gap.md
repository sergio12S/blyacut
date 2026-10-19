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
| Multiple video tracks / PiP / overlays | ✔ | Export rejects >1 video track | **P1** |
| Transitions | 100+ in categories | — | **P1** (crossfade, dip to black, wipe) |
| Speed, reverse, freeze frame, speed curves | ✔ | — | P2 |
| Keyframes (position/scale/opacity/volume) | ✔ | — | P2 |
| Crop/scale/position | ✔ | — | P2 (needs compositor) |
| Auto subtitles (speech-to-text) | ✔ | Planned (whisper.cpp) | P2 |
| Audio ducking | ✔ | Agent can set gains manually | P2 |
| Chroma key | ✔ | — | P3 |
| Motion / camera tracking, AI generation | ✔ | — | P3 / out of scope locally |
| Export presets (resolution, vertical, social) | ✔ | Fixed 1080p30 settings | P2 |
| Thumbnails/waveforms on timeline | ✔ | Decorative stripes | P2 |

## Roadmap

**Phase 1 — interface (now).** Library tabs (Media · Titles · Effects) with drag onto clips; tabbed inspector (Clip · Audio · Effects) with timecode fields; one compact header with undo/redo and export progress; no hint row; consistent Russian UI; title cards from the UI.

**Phase 2 — composition.** Multiple video tracks with overlay/PiP (position, scale, opacity), text overlay rendered as an image layer, transitions between adjacent clips, export presets. Preview parity first: the monitor must show what export renders.

**Phase 3 — motion and time.** Speed/reverse/freeze, keyframes, ducking, whisper.cpp subtitles, timeline thumbnails and waveforms.

Each phase keeps the rule that UI and agents share one Rust edit model: every new capability is an `Edit` operation and an MCP tool at the same time.

## Sources
- Wondershare, *Navigating Filmora*: https://filmora.wondershare.com/guide/filmora-navigating-filmora.html
- Wondershare, *Panel layout* (property panel, player): https://filmora.wondershare.de/guide/panel-layout.html
- Wondershare, *Features*: https://filmora.wondershare.com/filmora-features.html
- Wondershare, *What's new*: https://filmora.wondershare.com/whats-new-in-filmora-video-editor.html
- Wondershare, *Filmora for Mac*: https://filmora.wondershare.com/video-editor-mac/
- Movavi, *Wondershare Filmora review* (competitor review): https://www.movavi.com/adv/wondershare-filmora-review.html
