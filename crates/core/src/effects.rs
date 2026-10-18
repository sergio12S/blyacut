//! Portable effect recipes: data only, never shell commands, JavaScript or native plugins.
use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectRecipe {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub steps: Vec<EffectStep>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectStep {
    // Video
    Brightness {
        gain_milli: u16,
    },
    Contrast {
        gain_milli: u16,
    },
    Saturation {
        gain_milli: u16,
    },
    Hue {
        degrees: i16,
    },
    Gamma {
        gamma_milli: u16,
    },
    Vibrance {
        intensity_milli: i16,
    },
    Temperature {
        kelvin: u32,
    },
    Blur {
        strength: u16,
    },
    Sharpen {
        amount_milli: u16,
    },
    Vignette {
        strength_milli: u16,
    },
    FilmGrain {
        strength: u16,
    },
    Pixelate {
        block: u16,
    },
    Posterize {
        levels: u16,
    },
    ChromaticAberration {
        pixels: u16,
    },
    GhostTrails {
        decay_milli: u16,
    },
    FadeFromBlack {
        duration_ms: u32,
    },
    FadeToBlack {
        duration_ms: u32,
    },
    Grayscale {},
    Sepia {},
    Invert {},
    Mirror {},
    Flip {},
    EdgeDetect {},
    // Audio
    LowPass {
        frequency_hz: u32,
    },
    HighPass {
        frequency_hz: u32,
    },
    Echo {
        delay_ms: u32,
        decay_milli: u16,
    },
    Bass {
        gain_db: i16,
    },
    Treble {
        gain_db: i16,
    },
    Equalizer {
        frequency_hz: u32,
        gain_db: i16,
    },
    Reverb {
        room_milli: u16,
    },
    Compressor {
        threshold_db: i16,
        ratio: u16,
    },
    Pitch {
        semitones: i16,
    },
    Tremolo {
        frequency_milli: u32,
        depth_milli: u16,
    },
    Vibrato {
        frequency_milli: u32,
        depth_milli: u16,
    },
    AutoPan {
        frequency_milli: u32,
    },
    Bitcrush {
        bits: u16,
    },
    Denoise {
        strength_db: u16,
    },
    StereoWiden {
        amount_milli: u16,
    },
    Chorus {},
    Flanger {},
    Phaser {},
    Robot {},
    Normalize {},
}
/// Effect name, video flag and inclusive parameter ranges; drives validation and the MCP schema.
/// Parameter name with inclusive integer bounds.
pub type ParamRange = (&'static str, i64, i64);
pub const SPEC: &[(&str, bool, &[ParamRange])] = &[
    ("brightness", true, &[("gain_milli", 0, 2000)]),
    ("contrast", true, &[("gain_milli", 0, 3000)]),
    ("saturation", true, &[("gain_milli", 0, 3000)]),
    ("hue", true, &[("degrees", -360, 360)]),
    ("gamma", true, &[("gamma_milli", 100, 3000)]),
    ("vibrance", true, &[("intensity_milli", -2000, 2000)]),
    ("temperature", true, &[("kelvin", 1000, 40000)]),
    ("blur", true, &[("strength", 1, 50)]),
    ("sharpen", true, &[("amount_milli", 0, 3000)]),
    ("vignette", true, &[("strength_milli", 0, 1000)]),
    ("film_grain", true, &[("strength", 1, 100)]),
    ("pixelate", true, &[("block", 2, 64)]),
    ("posterize", true, &[("levels", 2, 32)]),
    ("chromatic_aberration", true, &[("pixels", 1, 40)]),
    ("ghost_trails", true, &[("decay_milli", 0, 990)]),
    ("fade_from_black", true, &[("duration_ms", 1, 60000)]),
    ("fade_to_black", true, &[("duration_ms", 1, 60000)]),
    ("grayscale", true, &[]),
    ("sepia", true, &[]),
    ("invert", true, &[]),
    ("mirror", true, &[]),
    ("flip", true, &[]),
    ("edge_detect", true, &[]),
    ("low_pass", false, &[("frequency_hz", 20, 20000)]),
    ("high_pass", false, &[("frequency_hz", 20, 20000)]),
    (
        "echo",
        false,
        &[("delay_ms", 1, 2000), ("decay_milli", 0, 900)],
    ),
    ("bass", false, &[("gain_db", -20, 20)]),
    ("treble", false, &[("gain_db", -20, 20)]),
    (
        "equalizer",
        false,
        &[("frequency_hz", 20, 20000), ("gain_db", -20, 20)],
    ),
    ("reverb", false, &[("room_milli", 0, 1000)]),
    (
        "compressor",
        false,
        &[("threshold_db", -60, 0), ("ratio", 1, 20)],
    ),
    ("pitch", false, &[("semitones", -12, 12)]),
    (
        "tremolo",
        false,
        &[("frequency_milli", 100, 20000), ("depth_milli", 0, 1000)],
    ),
    (
        "vibrato",
        false,
        &[("frequency_milli", 100, 20000), ("depth_milli", 0, 1000)],
    ),
    ("auto_pan", false, &[("frequency_milli", 10, 20000)]),
    ("bitcrush", false, &[("bits", 1, 16)]),
    ("denoise", false, &[("strength_db", 1, 90)]),
    ("stereo_widen", false, &[("amount_milli", 0, 10000)]),
    ("chorus", false, &[]),
    ("flanger", false, &[]),
    ("phaser", false, &[]),
    ("robot", false, &[]),
    ("normalize", false, &[]),
];
impl EffectStep {
    fn tagged(&self) -> (String, serde_json::Map<String, Value>) {
        let mut map = match serde_json::to_value(self) {
            Ok(Value::Object(map)) => map,
            _ => unreachable!("effect steps serialize as tagged objects"),
        };
        let name = match map.remove("effect") {
            Some(Value::String(name)) => name,
            _ => unreachable!("effect steps carry their tag"),
        };
        (name, map)
    }
    pub fn name(&self) -> String {
        self.tagged().0
    }
    pub fn is_video(&self) -> bool {
        let name = self.name();
        SPEC.iter().any(|(n, video, _)| *n == name && *video)
    }
    pub fn validate(&self) -> Result<()> {
        let (name, params) = self.tagged();
        let (_, _, ranges) = SPEC
            .iter()
            .find(|(n, ..)| *n == name)
            .ok_or("Unknown effect")?;
        for (param, min, max) in ranges.iter() {
            match params.get(*param).and_then(Value::as_i64) {
                Some(v) if (*min..=*max).contains(&v) => {}
                _ => return Err(format!("Effect {name}: {param} must be {min}..={max}")),
            }
        }
        Ok(())
    }
}
impl EffectRecipe {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1
            || self.id.is_empty()
            || self.id.len() > 64
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
            || self.name.is_empty()
            || self.name.len() > 128
            || self.steps.is_empty()
            || self.steps.len() > 16
        {
            return Err("Invalid effect recipe version, ID, name or step count".into());
        }
        for step in &self.steps {
            step.validate()?;
        }
        Ok(())
    }
}
/// This schema is also advertised by MCP; validation remains authoritative in Rust.
pub fn recipe_schema() -> Value {
    let steps: Vec<Value> = SPEC
        .iter()
        .map(|(name, _, ranges)| {
            let mut props = serde_json::Map::new();
            props.insert("effect".into(), json!({"type":"string","const":name}));
            let mut required = vec!["effect"];
            for (param, min, max) in ranges.iter() {
                props.insert(
                    (*param).into(),
                    json!({"type":"integer","minimum":min,"maximum":max}),
                );
                required.push(param);
            }
            json!({"type":"object","properties":props,"required":required,"additionalProperties":false})
        })
        .collect();
    json!({"type":"object","properties":{"version":{"const":1,"type":"integer"},"id":{"type":"string","pattern":"^[A-Za-z0-9_-]{1,64}$"},"name":{"type":"string","minLength":1,"maxLength":128},"steps":{"type":"array","minItems":1,"maxItems":16,"items":{"oneOf":steps}}},"required":["version","id","name","steps"],"additionalProperties":false})
}
/// Curated presets shipped with the editor. IDs start with `builtin-`.
pub fn builtin_recipes() -> Vec<EffectRecipe> {
    use EffectStep::*;
    let r = |id: &str, name: &str, steps: Vec<EffectStep>| EffectRecipe {
        version: 1,
        id: format!("builtin-{id}"),
        name: name.into(),
        steps,
    };
    vec![
        // Video looks
        r(
            "cinematic",
            "Кино",
            vec![
                Contrast { gain_milli: 1150 },
                Saturation { gain_milli: 850 },
                Temperature { kelvin: 5600 },
                Vignette {
                    strength_milli: 400,
                },
            ],
        ),
        r(
            "vintage",
            "Винтаж",
            vec![
                Sepia {},
                FilmGrain { strength: 18 },
                Vignette {
                    strength_milli: 600,
                },
            ],
        ),
        r(
            "noir",
            "Нуар",
            vec![
                Grayscale {},
                Contrast { gain_milli: 1450 },
                Vignette {
                    strength_milli: 700,
                },
            ],
        ),
        r(
            "vhs",
            "VHS",
            vec![
                ChromaticAberration { pixels: 5 },
                FilmGrain { strength: 25 },
                Saturation { gain_milli: 1300 },
                Blur { strength: 1 },
            ],
        ),
        r(
            "dream",
            "Сон",
            vec![
                Blur { strength: 3 },
                Brightness { gain_milli: 1100 },
                GhostTrails { decay_milli: 850 },
            ],
        ),
        r(
            "cyberpunk",
            "Киберпанк",
            vec![
                Temperature { kelvin: 11000 },
                Saturation { gain_milli: 1600 },
                Contrast { gain_milli: 1200 },
                ChromaticAberration { pixels: 3 },
            ],
        ),
        r(
            "pixel-art",
            "Пиксель-арт",
            vec![Pixelate { block: 12 }, Posterize { levels: 6 }],
        ),
        r(
            "comic",
            "Комикс",
            vec![
                Posterize { levels: 5 },
                Sharpen { amount_milli: 1500 },
                Saturation { gain_milli: 1400 },
            ],
        ),
        r(
            "warm",
            "Тёплый свет",
            vec![
                Temperature { kelvin: 3800 },
                Vibrance {
                    intensity_milli: 300,
                },
            ],
        ),
        r(
            "cold",
            "Холодный свет",
            vec![Temperature { kelvin: 9500 }, Contrast { gain_milli: 1080 }],
        ),
        r(
            "pop",
            "Сочные цвета",
            vec![
                Vibrance {
                    intensity_milli: 900,
                },
                Contrast { gain_milli: 1100 },
                Sharpen { amount_milli: 600 },
            ],
        ),
        r(
            "sketch",
            "Набросок",
            vec![EdgeDetect {}, Invert {}, Grayscale {}],
        ),
        r("negative", "Негатив", vec![Invert {}]),
        r("mirror", "Зеркало", vec![Mirror {}]),
        r("upside-down", "Вверх ногами", vec![Flip {}]),
        r(
            "psychedelic",
            "Психоделика",
            vec![
                Hue { degrees: 140 },
                Saturation { gain_milli: 2200 },
                GhostTrails { decay_milli: 900 },
            ],
        ),
        r(
            "fade-in",
            "Появление из чёрного",
            vec![FadeFromBlack { duration_ms: 1000 }],
        ),
        r(
            "fade-out",
            "Уход в чёрное",
            vec![FadeToBlack { duration_ms: 1000 }],
        ),
        // Audio
        r(
            "telephone",
            "Телефон",
            vec![
                HighPass { frequency_hz: 300 },
                LowPass { frequency_hz: 3400 },
                Bitcrush { bits: 10 },
            ],
        ),
        r(
            "radio",
            "Радио",
            vec![
                HighPass { frequency_hz: 500 },
                LowPass { frequency_hz: 4500 },
                Compressor {
                    threshold_db: -24,
                    ratio: 6,
                },
            ],
        ),
        r("cathedral", "Собор", vec![Reverb { room_milli: 950 }]),
        r("small-room", "Комната", vec![Reverb { room_milli: 300 }]),
        r("chipmunk", "Бурундук", vec![Pitch { semitones: 7 }]),
        r(
            "demon",
            "Демон",
            vec![
                Pitch { semitones: -7 },
                Bass { gain_db: 6 },
                Reverb { room_milli: 400 },
            ],
        ),
        r("robot", "Робот", vec![Robot {}]),
        r(
            "underwater",
            "Под водой",
            vec![
                LowPass { frequency_hz: 500 },
                Chorus {},
                Vibrato {
                    frequency_milli: 2000,
                    depth_milli: 300,
                },
            ],
        ),
        r(
            "podcast",
            "Чистый голос",
            vec![
                HighPass { frequency_hz: 80 },
                Denoise { strength_db: 12 },
                Compressor {
                    threshold_db: -20,
                    ratio: 4,
                },
                Normalize {},
            ],
        ),
        r(
            "bass-boost",
            "Мощный бас",
            vec![
                Bass { gain_db: 9 },
                Compressor {
                    threshold_db: -12,
                    ratio: 3,
                },
            ],
        ),
        r(
            "lofi",
            "Lo-fi",
            vec![
                Bitcrush { bits: 8 },
                LowPass { frequency_hz: 5000 },
                Tremolo {
                    frequency_milli: 500,
                    depth_milli: 150,
                },
            ],
        ),
        r(
            "stadium",
            "Стадион",
            vec![
                Echo {
                    delay_ms: 250,
                    decay_milli: 400,
                },
                Reverb { room_milli: 800 },
            ],
        ),
        r("chorus", "Хорус", vec![Chorus {}]),
        r("flanger", "Флэнжер", vec![Flanger {}]),
        r("phaser", "Фейзер", vec![Phaser {}]),
        r(
            "tremolo",
            "Тремоло",
            vec![Tremolo {
                frequency_milli: 6000,
                depth_milli: 600,
            }],
        ),
        r(
            "auto-pan",
            "Автопанорама",
            vec![AutoPan {
                frequency_milli: 500,
            }],
        ),
        r(
            "wide",
            "Широкое стерео",
            vec![StereoWiden { amount_milli: 2500 }],
        ),
    ]
}
