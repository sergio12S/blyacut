# Contributing

Use Node 22+ and Rust 1.90+ with FFmpeg installed. Run the checks in README before submitting changes. Keep the application runnable and document incomplete features clearly.

- Put editing behavior and its invariant tests in `crates/core`, never in Vue or an AI adapter.
- Keep FFmpeg argument generation separate from process execution. Never construct shell commands from media paths or project text.
- Add real synthetic-media integration checks for changes that affect export duration, streams or geometry.
- Preserve versioned JSON compatibility. Add explicit migrations when the schema changes; never silently reinterpret a newer project.
- Keep runtime dependencies small, local and license-compatible. Commit lockfiles.
- Report crashes with reproduction steps, OS version, tool versions and sanitized logs; do not attach private footage without permission.

Code is MIT licensed. Contributions must be yours to submit under that license.
