import type { EffectRecipe, EffectStep } from "./types";
import { isVideoStep } from "./types";

export interface EffectParamDef {
  key: string;
  name: string;
  min: number;
  max: number;
  default: number;
  step: number;
  unit: string;
  format?: (value: number) => string;
}

export interface EffectMeta {
  effect: EffectStep["effect"];
  kind: "video" | "audio";
  name: string;
  icon: string;
  category: string;
  description: string;
  params: EffectParamDef[];
  createDefaultStep: () => EffectStep;
}

export const EFFECTS_CATALOG: EffectMeta[] = [
  // --- Video: color & tone ---
  {
    effect: "brightness",
    kind: "video",
    name: "Brightness",
    icon: "☀️",
    category: "Color & tone",
    description: "Raise or lower overall brightness",
    params: [
      {
        key: "gain_milli",
        name: "Level",
        min: 0,
        max: 2000,
        default: 1000,
        step: 50,
        unit: "x",
        format: (v) => `${(v / 1000).toFixed(2)}x`,
      },
    ],
    createDefaultStep: () => ({ effect: "brightness", gain_milli: 1200 }),
  },
  {
    effect: "contrast",
    kind: "video",
    name: "Contrast",
    icon: "🌓",
    category: "Color & tone",
    description: "Difference between lights and darks",
    params: [
      {
        key: "gain_milli",
        name: "Level",
        min: 0,
        max: 3000,
        default: 1000,
        step: 50,
        unit: "x",
        format: (v) => `${(v / 1000).toFixed(2)}x`,
      },
    ],
    createDefaultStep: () => ({ effect: "contrast", gain_milli: 1250 }),
  },
  {
    effect: "saturation",
    kind: "video",
    name: "Saturation",
    icon: "🎨",
    category: "Color & tone",
    description: "Color intensity",
    params: [
      {
        key: "gain_milli",
        name: "Level",
        min: 0,
        max: 3000,
        default: 1000,
        step: 50,
        unit: "x",
        format: (v) => `${(v / 1000).toFixed(2)}x`,
      },
    ],
    createDefaultStep: () => ({ effect: "saturation", gain_milli: 1300 }),
  },
  {
    effect: "hue",
    kind: "video",
    name: "Hue",
    icon: "🌈",
    category: "Color & tone",
    description: "Rotate the color wheel",
    params: [
      {
        key: "degrees",
        name: "Angle",
        min: -360,
        max: 360,
        default: 0,
        step: 5,
        unit: "°",
        format: (v) => `${v}°`,
      },
    ],
    createDefaultStep: () => ({ effect: "hue", degrees: 45 }),
  },
  {
    effect: "gamma",
    kind: "video",
    name: "Gamma",
    icon: "⚡",
    category: "Color & tone",
    description: "Non-linear midtone correction",
    params: [
      {
        key: "gamma_milli",
        name: "Gamma",
        min: 100,
        max: 3000,
        default: 1000,
        step: 50,
        unit: "x",
        format: (v) => `${(v / 1000).toFixed(2)}x`,
      },
    ],
    createDefaultStep: () => ({ effect: "gamma", gamma_milli: 1200 }),
  },
  {
    effect: "vibrance",
    kind: "video",
    name: "Vibrance",
    icon: "✨",
    category: "Color & tone",
    description: "Boost muted colors without oversaturating skin",
    params: [
      {
        key: "intensity_milli",
        name: "Intensity",
        min: -2000,
        max: 2000,
        default: 0,
        step: 50,
        unit: "m",
        format: (v) => `${(v / 1000).toFixed(2)}`,
      },
    ],
    createDefaultStep: () => ({ effect: "vibrance", intensity_milli: 500 }),
  },
  {
    effect: "temperature",
    kind: "video",
    name: "Color temperature",
    icon: "🌡️",
    category: "Color & tone",
    description: "White balance: cool (>6500K) or warm (<6500K)",
    params: [
      {
        key: "kelvin",
        name: "Temperature",
        min: 1000,
        max: 40000,
        default: 6500,
        step: 250,
        unit: "K",
        format: (v) => `${v} K`,
      },
    ],
    createDefaultStep: () => ({ effect: "temperature", kelvin: 7500 }),
  },
  {
    effect: "grayscale",
    kind: "video",
    name: "Black & white",
    icon: "🔘",
    category: "Color & tone",
    description: "Turn the picture monochrome",
    params: [],
    createDefaultStep: () => ({ effect: "grayscale" }),
  },
  {
    effect: "sepia",
    kind: "video",
    name: "Sepia",
    icon: "📜",
    category: "Color & tone",
    description: "Warm vintage photo tone",
    params: [],
    createDefaultStep: () => ({ effect: "sepia" }),
  },
  {
    effect: "invert",
    kind: "video",
    name: "Invert colors",
    icon: "🔄",
    category: "Color & tone",
    description: "Negative: invert every color",
    params: [],
    createDefaultStep: () => ({ effect: "invert" }),
  },

  // --- Video: blur & sharpness ---
  {
    effect: "blur",
    kind: "video",
    name: "Blur",
    icon: "💧",
    category: "Blur & sharpness",
    description: "Soft Gaussian blur",
    params: [
      {
        key: "strength",
        name: "Strength",
        min: 1,
        max: 50,
        default: 5,
        step: 1,
        unit: "px",
        format: (v) => `${v} px`,
      },
    ],
    createDefaultStep: () => ({ effect: "blur", strength: 8 }),
  },
  {
    effect: "sharpen",
    kind: "video",
    name: "Sharpen",
    icon: "🗡️",
    category: "Blur & sharpness",
    description: "Crisper edges and detail",
    params: [
      {
        key: "amount_milli",
        name: "Strength",
        min: 0,
        max: 3000,
        default: 1000,
        step: 50,
        unit: "m",
        format: (v) => `${(v / 1000).toFixed(2)}x`,
      },
    ],
    createDefaultStep: () => ({ effect: "sharpen", amount_milli: 1200 }),
  },

  // --- Video: stylize & glitch ---
  {
    effect: "vignette",
    kind: "video",
    name: "Vignette",
    icon: "🔲",
    category: "Stylize & glitch",
    description: "Darken the edges to draw the eye to the center",
    params: [
      {
        key: "strength_milli",
        name: "Density",
        min: 0,
        max: 1000,
        default: 300,
        step: 25,
        unit: "m",
        format: (v) => `${Math.round(v / 10)}%`,
      },
    ],
    createDefaultStep: () => ({ effect: "vignette", strength_milli: 400 }),
  },
  {
    effect: "film_grain",
    kind: "video",
    name: "Film grain",
    icon: "🎞️",
    category: "Stylize & glitch",
    description: "Analog film noise and texture",
    params: [
      {
        key: "strength",
        name: "Intensity",
        min: 1,
        max: 100,
        default: 15,
        step: 1,
        unit: "%",
        format: (v) => `${v}%`,
      },
    ],
    createDefaultStep: () => ({ effect: "film_grain", strength: 20 }),
  },
  {
    effect: "pixelate",
    kind: "video",
    name: "Pixelate",
    icon: "👾",
    category: "Stylize & glitch",
    description: "Retro 8-bit blocks",
    params: [
      {
        key: "block",
        name: "Block size",
        min: 2,
        max: 64,
        default: 8,
        step: 1,
        unit: "px",
        format: (v) => `${v} px`,
      },
    ],
    createDefaultStep: () => ({ effect: "pixelate", block: 8 }),
  },
  {
    effect: "posterize",
    kind: "video",
    name: "Posterize",
    icon: "🖼️",
    category: "Stylize & glitch",
    description: "Fewer color steps, poster look",
    params: [
      {
        key: "levels",
        name: "Levels",
        min: 2,
        max: 32,
        default: 6,
        step: 1,
        unit: "lvl",
        format: (v) => `${v}`,
      },
    ],
    createDefaultStep: () => ({ effect: "posterize", levels: 6 }),
  },
  {
    effect: "chromatic_aberration",
    kind: "video",
    name: "Chromatic aberration",
    icon: "👓",
    category: "Stylize & glitch",
    description: "RGB channel split (lens glitch)",
    params: [
      {
        key: "pixels",
        name: "Shift",
        min: 1,
        max: 40,
        default: 4,
        step: 1,
        unit: "px",
        format: (v) => `${v} px`,
      },
    ],
    createDefaultStep: () => ({ effect: "chromatic_aberration", pixels: 5 }),
  },
  {
    effect: "edge_detect",
    kind: "video",
    name: "Edge detect",
    icon: "📐",
    category: "Stylize & glitch",
    description: "Outline the edges of objects",
    params: [],
    createDefaultStep: () => ({ effect: "edge_detect" }),
  },

  // --- Video: transform & fades ---
  {
    effect: "mirror",
    kind: "video",
    name: "Mirror horizontal",
    icon: "🪞",
    category: "Transform",
    description: "Mirror left-right",
    params: [],
    createDefaultStep: () => ({ effect: "mirror" }),
  },
  {
    effect: "flip",
    kind: "video",
    name: "Flip vertical",
    icon: "🙃",
    category: "Transform",
    description: "Turn the frame upside down",
    params: [],
    createDefaultStep: () => ({ effect: "flip" }),
  },
  {
    effect: "ghost_trails",
    kind: "video",
    name: "Ghost trails",
    icon: "👻",
    category: "Transform",
    description: "Trails behind moving objects",
    params: [
      {
        key: "decay_milli",
        name: "Decay",
        min: 0,
        max: 990,
        default: 500,
        step: 25,
        unit: "m",
        format: (v) => `${Math.round(v / 10)}%`,
      },
    ],
    createDefaultStep: () => ({ effect: "ghost_trails", decay_milli: 600 }),
  },
  {
    effect: "fade_from_black",
    kind: "video",
    name: "Fade from black",
    icon: "🎬",
    category: "Fades",
    description: "Fade in from black at the clip start",
    params: [
      {
        key: "duration_ms",
        name: "Duration",
        min: 1,
        max: 60000,
        default: 1000,
        step: 100,
        unit: "ms",
        format: (v) => `${(v / 1000).toFixed(1)} s`,
      },
    ],
    createDefaultStep: () => ({ effect: "fade_from_black", duration_ms: 1000 }),
  },
  {
    effect: "fade_to_black",
    kind: "video",
    name: "Fade to black",
    icon: "🌑",
    category: "Fades",
    description: "Fade out to black at the clip end",
    params: [
      {
        key: "duration_ms",
        name: "Duration",
        min: 1,
        max: 60000,
        default: 1000,
        step: 100,
        unit: "ms",
        format: (v) => `${(v / 1000).toFixed(1)} s`,
      },
    ],
    createDefaultStep: () => ({ effect: "fade_to_black", duration_ms: 1000 }),
  },

  // --- Audio: frequencies & EQ ---
  {
    effect: "low_pass",
    kind: "audio",
    name: "Low-pass",
    icon: "🔉",
    category: "Frequencies & EQ",
    description: "Cuts highs: the 'sound behind a door' effect",
    params: [
      {
        key: "frequency_hz",
        name: "Cutoff",
        min: 20,
        max: 20000,
        default: 1200,
        step: 50,
        unit: "Hz",
        format: (v) => (v >= 1000 ? `${(v / 1000).toFixed(1)} kHz` : `${v} Hz`),
      },
    ],
    createDefaultStep: () => ({ effect: "low_pass", frequency_hz: 1200 }),
  },
  {
    effect: "high_pass",
    kind: "audio",
    name: "High-pass",
    icon: "📻",
    category: "Frequencies & EQ",
    description: "Removes rumble and boom for clean voice",
    params: [
      {
        key: "frequency_hz",
        name: "Cutoff",
        min: 20,
        max: 20000,
        default: 100,
        step: 10,
        unit: "Hz",
        format: (v) => (v >= 1000 ? `${(v / 1000).toFixed(1)} kHz` : `${v} Hz`),
      },
    ],
    createDefaultStep: () => ({ effect: "high_pass", frequency_hz: 85 }),
  },
  {
    effect: "bass",
    kind: "audio",
    name: "Bass",
    icon: "🔊",
    category: "Frequencies & EQ",
    description: "Boost or cut bass (100 Hz)",
    params: [
      {
        key: "gain_db",
        name: "Gain",
        min: -20,
        max: 20,
        default: 0,
        step: 0.5,
        unit: "dB",
        format: (v) => `${v > 0 ? "+" : ""}${v} dB`,
      },
    ],
    createDefaultStep: () => ({ effect: "bass", gain_db: 4 }),
  },
  {
    effect: "treble",
    kind: "audio",
    name: "Treble",
    icon: "🔔",
    category: "Frequencies & EQ",
    description: "Boost or cut treble (10 kHz)",
    params: [
      {
        key: "gain_db",
        name: "Gain",
        min: -20,
        max: 20,
        default: 0,
        step: 0.5,
        unit: "dB",
        format: (v) => `${v > 0 ? "+" : ""}${v} dB`,
      },
    ],
    createDefaultStep: () => ({ effect: "treble", gain_db: 3 }),
  },
  {
    effect: "equalizer",
    kind: "audio",
    name: "Parametric EQ",
    icon: "🎚️",
    category: "Frequencies & EQ",
    description: "Precise gain at one frequency",
    params: [
      {
        key: "frequency_hz",
        name: "Frequency",
        min: 20,
        max: 20000,
        default: 1000,
        step: 50,
        unit: "Hz",
        format: (v) => (v >= 1000 ? `${(v / 1000).toFixed(1)} kHz` : `${v} Hz`),
      },
      {
        key: "gain_db",
        name: "Gain",
        min: -20,
        max: 20,
        default: 0,
        step: 0.5,
        unit: "dB",
        format: (v) => `${v > 0 ? "+" : ""}${v} dB`,
      },
    ],
    createDefaultStep: () => ({
      effect: "equalizer",
      frequency_hz: 3000,
      gain_db: 3,
    }),
  },

  // --- Audio: space & dynamics ---
  {
    effect: "echo",
    kind: "audio",
    name: "Echo / delay",
    icon: "⛰️",
    category: "Space & delay",
    description: "Rhythmic decaying repeats",
    params: [
      {
        key: "delay_ms",
        name: "Delay",
        min: 1,
        max: 2000,
        default: 250,
        step: 10,
        unit: "ms",
        format: (v) => `${v} ms`,
      },
      {
        key: "decay_milli",
        name: "Decay",
        min: 0,
        max: 900,
        default: 400,
        step: 25,
        unit: "m",
        format: (v) => `${Math.round(v / 10)}%`,
      },
    ],
    createDefaultStep: () => ({
      effect: "echo",
      delay_ms: 240,
      decay_milli: 350,
    }),
  },
  {
    effect: "reverb",
    kind: "audio",
    name: "Reverb",
    icon: "🏛️",
    category: "Space & delay",
    description: "Hall or room acoustics",
    params: [
      {
        key: "room_milli",
        name: "Room size",
        min: 0,
        max: 1000,
        default: 500,
        step: 25,
        unit: "m",
        format: (v) => `${Math.round(v / 10)}%`,
      },
    ],
    createDefaultStep: () => ({ effect: "reverb", room_milli: 450 }),
  },
  {
    effect: "auto_pan",
    kind: "audio",
    name: "Auto-pan",
    icon: "↔️",
    category: "Space & delay",
    description: "Sound sweeps between left and right",
    params: [
      {
        key: "frequency_milli",
        name: "Rate",
        min: 10,
        max: 20000,
        default: 500,
        step: 50,
        unit: "mHz",
        format: (v) => `${(v / 1000).toFixed(2)} Hz`,
      },
    ],
    createDefaultStep: () => ({ effect: "auto_pan", frequency_milli: 600 }),
  },
  {
    effect: "stereo_widen",
    kind: "audio",
    name: "Stereo widen",
    icon: "🎧",
    category: "Space & delay",
    description: "Wider stereo image",
    params: [
      {
        key: "amount_milli",
        name: "Width",
        min: 0,
        max: 10000,
        default: 1000,
        step: 100,
        unit: "m",
        format: (v) => `${(v / 1000).toFixed(1)}x`,
      },
    ],
    createDefaultStep: () => ({ effect: "stereo_widen", amount_milli: 1500 }),
  },
  {
    effect: "compressor",
    kind: "audio",
    name: "Compressor",
    icon: "🗜️",
    category: "Dynamics & level",
    description: "Evens out dynamics (whispers louder, peaks softer)",
    params: [
      {
        key: "threshold_db",
        name: "Threshold",
        min: -60,
        max: 0,
        default: -18,
        step: 1,
        unit: "dB",
        format: (v) => `${v} dB`,
      },
      {
        key: "ratio",
        name: "Ratio",
        min: 1,
        max: 20,
        default: 4,
        step: 0.5,
        unit: ":1",
        format: (v) => `${v}:1`,
      },
    ],
    createDefaultStep: () => ({
      effect: "compressor",
      threshold_db: -20,
      ratio: 4,
    }),
  },
  {
    effect: "denoise",
    kind: "audio",
    name: "Noise reduction",
    icon: "🧹",
    category: "Dynamics & level",
    description: "Suppress steady background mic noise",
    params: [
      {
        key: "strength_db",
        name: "Reduction",
        min: 1,
        max: 90,
        default: 20,
        step: 1,
        unit: "dB",
        format: (v) => `${v} dB`,
      },
    ],
    createDefaultStep: () => ({ effect: "denoise", strength_db: 24 }),
  },
  {
    effect: "normalize",
    kind: "audio",
    name: "Normalize",
    icon: "📊",
    category: "Dynamics & level",
    description: "Even out loudness without distortion",
    params: [],
    createDefaultStep: () => ({ effect: "normalize" }),
  },

  // --- Audio: modulation ---
  {
    effect: "pitch",
    kind: "audio",
    name: "Pitch",
    icon: "🎵",
    category: "Modulation",
    description: "Shift the pitch up or down",
    params: [
      {
        key: "semitones",
        name: "Semitones",
        min: -12,
        max: 12,
        default: 0,
        step: 1,
        unit: "st",
        format: (v) => `${v > 0 ? "+" : ""}${v} st`,
      },
    ],
    createDefaultStep: () => ({ effect: "pitch", semitones: 2 }),
  },
  {
    effect: "tremolo",
    kind: "audio",
    name: "Tremolo",
    icon: "〰️",
    category: "Modulation",
    description: "Periodic volume pulsing",
    params: [
      {
        key: "frequency_milli",
        name: "Frequency",
        min: 100,
        max: 20000,
        default: 4000,
        step: 100,
        unit: "mHz",
        format: (v) => `${(v / 1000).toFixed(1)} Hz`,
      },
      {
        key: "depth_milli",
        name: "Depth",
        min: 0,
        max: 1000,
        default: 500,
        step: 25,
        unit: "m",
        format: (v) => `${Math.round(v / 10)}%`,
      },
    ],
    createDefaultStep: () => ({
      effect: "tremolo",
      frequency_milli: 4000,
      depth_milli: 600,
    }),
  },
  {
    effect: "vibrato",
    kind: "audio",
    name: "Vibrato",
    icon: "📯",
    category: "Modulation",
    description: "Periodic pitch wobble",
    params: [
      {
        key: "frequency_milli",
        name: "Frequency",
        min: 100,
        max: 20000,
        default: 5000,
        step: 100,
        unit: "mHz",
        format: (v) => `${(v / 1000).toFixed(1)} Hz`,
      },
      {
        key: "depth_milli",
        name: "Depth",
        min: 0,
        max: 1000,
        default: 400,
        step: 25,
        unit: "m",
        format: (v) => `${Math.round(v / 10)}%`,
      },
    ],
    createDefaultStep: () => ({
      effect: "vibrato",
      frequency_milli: 5000,
      depth_milli: 400,
    }),
  },
  {
    effect: "bitcrush",
    kind: "audio",
    name: "Bitcrush",
    icon: "🕹️",
    category: "Modulation",
    description: "Lower bit depth for a retro console sound",
    params: [
      {
        key: "bits",
        name: "Bit depth",
        min: 1,
        max: 16,
        default: 8,
        step: 1,
        unit: "bit",
        format: (v) => `${v} bit`,
      },
    ],
    createDefaultStep: () => ({ effect: "bitcrush", bits: 8 }),
  },
  {
    effect: "chorus",
    kind: "audio",
    name: "Chorus",
    icon: "👥",
    category: "Modulation",
    description: "Ensemble: many voices at once",
    params: [],
    createDefaultStep: () => ({ effect: "chorus" }),
  },
  {
    effect: "flanger",
    kind: "audio",
    name: "Flanger",
    icon: "✈️",
    category: "Modulation",
    description: "Sweeping jet whoosh",
    params: [],
    createDefaultStep: () => ({ effect: "flanger" }),
  },
  {
    effect: "phaser",
    kind: "audio",
    name: "Phaser",
    icon: "🌀",
    category: "Modulation",
    description: "Phasing swirl",
    params: [],
    createDefaultStep: () => ({ effect: "phaser" }),
  },
  {
    effect: "robot",
    kind: "audio",
    name: "Robot voice",
    icon: "🤖",
    category: "Modulation",
    description: "Metallic vocoder timbre",
    params: [],
    createDefaultStep: () => ({ effect: "robot" }),
  },
];

const CATALOG_BY_EFFECT = new Map<EffectStep["effect"], EffectMeta>(
  EFFECTS_CATALOG.map((item) => [item.effect, item]),
);

export function getEffectMeta(
  effectName: EffectStep["effect"],
): EffectMeta | undefined {
  return CATALOG_BY_EFFECT.get(effectName);
}

/** Formats a single EffectStep into a readable Russian summary */
export function formatEffectStep(step: EffectStep): string {
  const meta = getEffectMeta(step.effect);
  if (!meta) return step.effect;

  if (meta.params.length === 0) {
    return `${meta.icon} ${meta.name}`;
  }

  const parts: string[] = [];
  for (const p of meta.params) {
    const rawVal = (step as Record<string, unknown>)[p.key];
    if (typeof rawVal === "number") {
      parts.push(p.format ? p.format(rawVal) : `${rawVal}${p.unit}`);
    }
  }

  return `${meta.icon} ${meta.name}${parts.length ? ` (${parts.join(", ")})` : ""}`;
}

/** Formats an entire EffectRecipe into a sequence of steps */
export function formatRecipeSteps(recipe: EffectRecipe): string {
  return recipe.steps
    .map((s) => {
      const meta = getEffectMeta(s.effect);
      return meta ? `${meta.icon} ${meta.name}` : s.effect;
    })
    .join(" → ");
}

/** Returns true if all steps in the recipe are compatible with audio tracks */
export function isAudioCompatibleRecipe(recipe: EffectRecipe): boolean {
  return recipe.steps.every((step) => !isVideoStep(step));
}
