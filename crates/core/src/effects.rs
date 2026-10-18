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
    Brightness { gain_milli: u16 },
    Contrast { gain_milli: u16 },
    Saturation { gain_milli: u16 },
    Hue { degrees: i16 },
    LowPass { frequency_hz: u32 },
    HighPass { frequency_hz: u32 },
    Echo { delay_ms: u32, decay_milli: u16 },
}
impl EffectStep {
    pub fn is_video(&self) -> bool {
        matches!(
            self,
            Self::Brightness { .. }
                | Self::Contrast { .. }
                | Self::Saturation { .. }
                | Self::Hue { .. }
        )
    }
    pub fn validate(&self) -> Result<()> {
        let valid = match self {
            Self::Brightness { gain_milli } => *gain_milli <= 2000,
            Self::Contrast { gain_milli } | Self::Saturation { gain_milli } => *gain_milli <= 3000,
            Self::Hue { degrees } => (-360..=360).contains(degrees),
            Self::LowPass { frequency_hz } | Self::HighPass { frequency_hz } => {
                (20..=20000).contains(frequency_hz)
            }
            Self::Echo {
                delay_ms,
                decay_milli,
            } => (1..=2000).contains(delay_ms) && *decay_milli <= 900,
        };
        if valid {
            Ok(())
        } else {
            Err("Effect parameter outside supported range".into())
        }
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
    fn step(name: &str, properties: Value, required: &[&str]) -> Value {
        let mut props = properties.as_object().unwrap().clone();
        props.insert("effect".into(), json!({"type":"string","const":name}));
        let mut keys = vec!["effect"];
        keys.extend(required);
        json!({"type":"object","properties":props,"required":keys,"additionalProperties":false})
    }
    let range = |min: i32, max: i32| json!({"type":"integer","minimum":min,"maximum":max});
    json!({"type":"object","properties":{"version":{"const":1,"type":"integer"},"id":{"type":"string","pattern":"^[A-Za-z0-9_-]{1,64}$"},"name":{"type":"string","minLength":1,"maxLength":128},"steps":{"type":"array","minItems":1,"maxItems":16,"items":{"oneOf":[
 step("brightness",json!({"gain_milli":range(0,2000)}),&["gain_milli"]),
 step("contrast",json!({"gain_milli":range(0,3000)}),&["gain_milli"]),
 step("saturation",json!({"gain_milli":range(0,3000)}),&["gain_milli"]),
 step("hue",json!({"degrees":range(-360,360)}),&["degrees"]),
 step("low_pass",json!({"frequency_hz":range(20,20000)}),&["frequency_hz"]),
 step("high_pass",json!({"frequency_hz":range(20,20000)}),&["frequency_hz"]),
 step("echo",json!({"delay_ms":range(1,2000),"decay_milli":range(0,900)}),&["delay_ms","decay_milli"])
 ]}}},"required":["version","id","name","steps"],"additionalProperties":false})
}
