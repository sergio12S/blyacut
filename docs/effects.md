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

| Step | Parameters |
|---|---|
| `brightness` | `gain_milli`: 0–2000; 1000 unity |
| `contrast`, `saturation` | `gain_milli`: 0–3000; 1000 unity |
| `hue` | `degrees`: −360…360 |
| `low_pass`, `high_pass` | `frequency_hz`: 20–20000 |
| `echo` | `delay_ms`: 1–2000; `decay_milli`: 0–900 |

Steps run in order within their video/audio domain. Audio filters precede clip gain/fades; echo tails are clipped at the clip boundary, keeping timeline duration fixed. At most four recipes and sixteen total steps per clip. Audio tracks accept audio steps only. Recipe IDs are 1–64 ASCII letters/digits/underscore/hyphen, names 1–128 bytes. Unknown fields, operations and out-of-range values are rejected atomically. Catalog files are bounded to 64 KiB, with up to 128 directory entries, warnings for invalid files and canonical path containment checks.

FFmpeg export uses fixed argument/filter templates with numeric parameters. Preview uses CSS color filters and Web Audio biquad/feedforward delay nodes. These algorithms are approximate: CSS brightness is multiplicative; contrast, saturation and hue can differ from FFmpeg color processing. Audio resampling/filter state, synchronization, cuts and the final mix limiter also differ. Export tests measure real decoded grayscale pixels, low-pass attenuation and echo delay/gain; a native QA run also displayed a monochrome frame after a live effect edit. Other native color effects and auditory DSP preview remain unverified. Use short exports to judge the final result.

Recipes can combine the seven existing primitives into new looks and sounds. They cannot yet introduce a new processing algorithm. Programmable shaders, WASM DSP, LUTs, arbitrary FFmpeg graphs, generated native code, parametric animation and a plugin marketplace are **not implemented**. A future executable-effect contract must define resource limits, versioning and equivalent preview/export behavior before exposing it to agents.
