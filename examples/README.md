# Example

Run `python3 scripts/generate-example.py` from the repository. It creates a six-second synthetic video (moving test pattern and tone); no footage download is required. The command refuses to overwrite an existing fixture.

Open `first-cut.blyacut`. The timeline contains source seconds 0–2 and 4–6. Its expected exported duration is approximately four seconds at 640×360, 30 fps. This also demonstrates project-relative media paths.

`generate-example.py` also generates `blue.mp4` and `soundtrack.wav` (a synthetic test tone, not generated music). It preserves existing files. `agent-audio-demo.blyacut` was saved from the desktop after an external MCP client assembled two two-second video clips plus a four-second audio track at 25% gain, with 500 ms fades. Generate the assets before opening it. See `docs/mcp.md` for a reproducible external-client workflow.

`effects/` contains hot-loadable Recipe v1 JSON chains. `qa-effects-demo.blyacut` is the schema-v3 native QA project with embedded monochrome/low-pass effects; `qa-effects-export.mp4` was exported before the subsequent review changed soundtrack gain from 25% to 30%. Binary fixtures/exports are generated local artifacts and are not committed. See `docs/verification.md`.
