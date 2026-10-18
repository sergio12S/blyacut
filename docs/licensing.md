# Licensing and distribution status

BlyaCut's own source is MIT licensed. The [generated dependency inventory](dependency-licenses.md) records declared package licenses, including transitive dependencies and platform/build-time packages. It is an inventory for review, not a certification of every bundled artifact. Regenerate with `npm ci && python3 scripts/license-inventory.py`.

Direct Rust dependencies: Tauri/tauri-build/dialog plugin (MIT OR Apache-2.0), Serde/serde_json (MIT OR Apache-2.0), UUID (Apache-2.0 OR MIT). Direct frontend libraries Vue, Pinia and Tauri JS packages use MIT; see the inventory for exact installed package declarations and development tooling.

FFmpeg is an external executable, not linked into the editor or bundled by this repository. The MVP requests `libx264` and the native AAC encoder. FFmpeg's license depends on its configuration: enabling GPL components such as libx264 makes that FFmpeg build GPL. Builds with `--enable-nonfree` cannot be redistributed. See [FFmpeg's license](https://ffmpeg.org/doxygen/trunk/md_LICENSE.html) and [distribution guidance](https://ffmpeg.org/legal.html).

Before distributing any FFmpeg executable, record `ffmpeg -version` and `ffmpeg -buildconf`, verify every enabled component, ship the required notices/licenses and corresponding source or source offer as applicable, and review the complete distribution arrangement. This project does not provide a pre-approved binary bundle. Codec patent obligations may be separate from source-code licensing and vary by jurisdiction; they have not been cleared for a public binary release.

No third-party footage, model weights, fonts or commercial assets are bundled. Test fixtures are generated from FFmpeg test sources. whisper.cpp/model licensing must be evaluated when that feature is added. The development .app is unsigned and not a public release artifact.
