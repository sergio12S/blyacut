# Hot-loadable effect recipes

Recipes are **data**, not executable plugins. Add JSON files to a folder and click **Load recipe folder… / Refresh recipes** in the clip inspector. No rebuild, reinstall or restart is needed. Applied recipes are copied into the project (schema v3), so later catalog changes cannot silently change a saved edit. Loading v1/v2 projects supplies an empty effect list.

See `examples/effects/` for grayscale/contrast and telephone/echo chains. For agents, authorize the parent folder containing `effects/`; `create_effect_recipe({recipe})` writes a new `<id>.json` there without overwriting. `list_effect_recipes({})` rereads the folder. Apply the returned recipe through `apply_edits` or `propose_edits`, using the same operation as the UI:

```json
{
  "expected_revision": 7,
  "edits": [{
    "operation": "set_effects",
    "clip_id": "existing-clip-id",
    "effects": [{
      "version": 1,
      "id": "mono",
      "name": "Monochrome",
      "steps": [{"effect": "saturation", "gain_milli": 0}]
    }]
  }]
}
```

## Built-in presets

36 curated presets ship inside the binary (`blyacut_core::effects::builtin_recipes`, IDs `builtin-*`): Tauri `builtin_effect_recipes`, and the `builtin` field of MCP `list_effect_recipes`. Video looks: Cinematic, Vintage, Noir, VHS, Dream, Cyberpunk, Pixel art, Comic, Warm/Cold light, Pop colors, Sketch, Negative, Mirror, Upside down, Psychedelic, Fade in from black, Fade out to black. Audio: Telephone, Radio, Cathedral, Small room, Chipmunk, Demon, Robot, Underwater, Clean voice, Bass boost, Lo-fi, Stadium, Chorus, Flanger, Phaser, Tremolo, Auto-pan, Wide stereo. Each preset is a normal recipe; copy and edit it freely.

## Steps (43)

`blyacut_core::effects::SPEC` is the single table behind validation and the MCP JSON Schema. All parameters are integers with inclusive bounds.

| Video step | Parameters | FFmpeg operator |
|---|---|---|
| `brightness` | `gain_milli` 0–2000 (1000 unity) | `colorchannelmixer` |
| `contrast`, `saturation` | `gain_milli` 0–3000 | `eq` |
| `hue` | `degrees` −360…360 | `hue` |
| `gamma` | `gamma_milli` 100–3000 | `eq=gamma` |
| `vibrance` | `intensity_milli` −2000…2000 | `vibrance` |
| `temperature` | `kelvin` 1000–40000 (6500 neutral) | `colortemperature` |
| `blur` | `strength` 1–50 | `gblur` |
| `sharpen` | `amount_milli` 0–3000 | `unsharp` |
| `vignette` | `strength_milli` 0–1000 | `vignette` |
| `film_grain` | `strength` 1–100 | `noise` |
| `pixelate` | `block` 2–64 | `pixelize` |
| `posterize` | `levels` 2–32 | `lutrgb` |
| `chromatic_aberration` | `pixels` 1–40 | `rgbashift` |
| `ghost_trails` | `decay_milli` 0–990 | `lagfun` |
| `fade_from_black`, `fade_to_black` | `duration_ms` 1–60000 (clamped to clip) | `fade` |
| `grayscale`, `sepia`, `invert`, `mirror`, `flip`, `edge_detect` | — | `hue=s=0`, `colorchannelmixer`, `negate`, `hflip`, `vflip`, `edgedetect` |

| Audio step | Parameters | FFmpeg operator |
|---|---|---|
| `low_pass`, `high_pass` | `frequency_hz` 20–20000 | `lowpass`/`highpass` |
| `echo` | `delay_ms` 1–2000, `decay_milli` 0–900 | `aecho` |
| `bass`, `treble` | `gain_db` −20…20 | `bass`/`treble` |
| `equalizer` | `frequency_hz` 20–20000, `gain_db` −20…20 | `equalizer` |
| `reverb` | `room_milli` 0–1000 | multi-tap `aecho` |
| `compressor` | `threshold_db` −60…0, `ratio` 1–20 | `acompressor` |
| `pitch` | `semitones` −12…12 (duration kept) | `asetrate`+`atempo` |
| `tremolo`, `vibrato` | `frequency_milli` 100–20000, `depth_milli` 0–1000 | `tremolo`/`vibrato` |
| `auto_pan` | `frequency_milli` 10–20000 | `apulsator` |
| `bitcrush` | `bits` 1–16 | `acrusher` |
| `denoise` | `strength_db` 1–90 | `afftdn` |
| `stereo_widen` | `amount_milli` 0–10000 | `extrastereo` |
| `chorus`, `flanger`, `phaser`, `robot`, `normalize` | — | `chorus`, `flanger`, `aphaser`, `afftfilt`, `dynaudnorm` |

`crates/engine/tests/effects_pack.rs` runs every step at its minimum and maximum through real FFmpeg (as `-vf`/`-af` and inside `-filter_complex`) and renders a project with stacked built-in presets end to end.

Steps run in order within their video/audio domain. Audio filters precede clip gain/fades; echo tails are clipped at the clip boundary, keeping timeline duration fixed. At most eight recipes and 32 total steps per clip. Audio tracks accept audio steps only. Recipe IDs are 1–64 ASCII letters/digits/underscore/hyphen, names 1–128 bytes. Unknown fields, operations and out-of-range values are rejected atomically. Catalog files are bounded to 64 KiB, with up to 128 directory entries, warnings for invalid files and canonical path containment checks.

FFmpeg export uses fixed argument/filter templates with numeric parameters. Preview uses CSS filters (color, blur, grayscale/sepia/invert, mirror/flip) and Web Audio biquad/feedforward delay nodes; the other steps are export-only and marked as approximate in the monitor. These algorithms are approximate: CSS brightness is multiplicative; contrast, saturation and hue can differ from FFmpeg color processing. Audio resampling/filter state, synchronization, cuts and the final mix limiter also differ. Export tests measure real decoded grayscale pixels, low-pass attenuation and echo delay/gain; a native QA run also displayed a monochrome frame after a live effect edit. Other native color effects and auditory DSP preview remain unverified. Use short exports to judge the final result.

Recipes can combine the 43 existing steps into new looks and sounds. They cannot yet introduce a new processing algorithm. Programmable shaders, WASM DSP, LUTs, arbitrary FFmpeg graphs, generated native code, parametric animation and a plugin marketplace are **not implemented**. A future executable-effect contract must define resource limits, versioning and equivalent preview/export behavior before exposing it to agents.
