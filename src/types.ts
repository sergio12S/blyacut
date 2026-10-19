export interface Asset {
  id: string;
  name: string;
  path: string;
  duration_ms: number;
  width: number;
  height: number;
  has_audio: boolean;
  /** A single image, usable for up to an hour. */
  still?: boolean;
}
/** Overlay placement in thousandths (centre x/y of the frame, scale vs. fitting the frame, opacity). */
export interface Transform {
  x_milli: number;
  y_milli: number;
  scale_milli: number;
  opacity_milli: number;
}
export interface Keyframe {
  at_ms: number;
  transform: Transform;
}
/** Overlay placement `local` ms into the clip (mirrors Rust `Clip::transform_at`). */
export function transformAt(clip: Clip, local: number): Transform {
  const keys = clip.keyframes ?? [];
  if (!keys.length) return clip.transform ?? FULL_FRAME;
  if (local <= keys[0].at_ms) return keys[0].transform;
  for (let i = 1; i < keys.length; i++) {
    const [a, b] = [keys[i - 1], keys[i]];
    if (local <= b.at_ms) {
      const f = (local - a.at_ms) / Math.max(1, b.at_ms - a.at_ms);
      const mix = (x: number, y: number) => Math.round(x + (y - x) * f);
      return {
        x_milli: mix(a.transform.x_milli, b.transform.x_milli),
        y_milli: mix(a.transform.y_milli, b.transform.y_milli),
        scale_milli: mix(a.transform.scale_milli, b.transform.scale_milli),
        opacity_milli: mix(
          a.transform.opacity_milli,
          b.transform.opacity_milli,
        ),
      };
    }
  }
  return keys[keys.length - 1].transform;
}
export const FULL_FRAME: Transform = {
  x_milli: 500,
  y_milli: 500,
  scale_milli: 1000,
  opacity_milli: 1000,
};
/** Mirrors `blyacut_core::effects::EffectStep`; Rust validation is authoritative. */
export type EffectStep =
  // Video
  | { effect: "brightness" | "contrast" | "saturation"; gain_milli: number }
  | { effect: "hue"; degrees: number }
  | { effect: "gamma"; gamma_milli: number }
  | { effect: "vibrance"; intensity_milli: number }
  | { effect: "temperature"; kelvin: number }
  | { effect: "blur"; strength: number }
  | { effect: "sharpen"; amount_milli: number }
  | { effect: "vignette"; strength_milli: number }
  | { effect: "film_grain"; strength: number }
  | { effect: "pixelate"; block: number }
  | { effect: "posterize"; levels: number }
  | { effect: "chromatic_aberration"; pixels: number }
  | { effect: "ghost_trails"; decay_milli: number }
  | { effect: "fade_from_black" | "fade_to_black"; duration_ms: number }
  | {
      effect:
        "grayscale" | "sepia" | "invert" | "mirror" | "flip" | "edge_detect";
    }
  // Audio
  | { effect: "low_pass" | "high_pass"; frequency_hz: number }
  | { effect: "echo"; delay_ms: number; decay_milli: number }
  | { effect: "bass" | "treble"; gain_db: number }
  | { effect: "equalizer"; frequency_hz: number; gain_db: number }
  | { effect: "reverb"; room_milli: number }
  | { effect: "compressor"; threshold_db: number; ratio: number }
  | { effect: "pitch"; semitones: number }
  | {
      effect: "tremolo" | "vibrato";
      frequency_milli: number;
      depth_milli: number;
    }
  | { effect: "auto_pan"; frequency_milli: number }
  | { effect: "bitcrush"; bits: number }
  | { effect: "denoise"; strength_db: number }
  | { effect: "stereo_widen"; amount_milli: number }
  | {
      effect: "chorus" | "flanger" | "phaser" | "robot" | "normalize";
    };
export const VIDEO_EFFECTS = new Set<EffectStep["effect"]>([
  "brightness",
  "contrast",
  "saturation",
  "hue",
  "gamma",
  "vibrance",
  "temperature",
  "blur",
  "sharpen",
  "vignette",
  "film_grain",
  "pixelate",
  "posterize",
  "chromatic_aberration",
  "ghost_trails",
  "fade_from_black",
  "fade_to_black",
  "grayscale",
  "sepia",
  "invert",
  "mirror",
  "flip",
  "edge_detect",
]);
/** Default track names are stored in English in projects; show them localized. */
export const trackName = (name: string) =>
  name
    .replace(/^Video (\d+)$/, "Видео $1")
    .replace(/^Audio (\d+)$/, "Звук $1")
    .replace(/^Subtitles$/, "Субтитры");
/** The first video track is the magnetic main track; later video tracks are overlays. */
export const isOverlayTrack = (project: Project | undefined, track: Track) =>
  track.kind === "video" &&
  project?.tracks.find((t) => t.kind === "video")?.id !== track.id;
/** Drag-and-drop payload: a JSON EffectRecipe dropped onto a timeline clip. */
export const RECIPE_MIME = "application/x-blyacut-recipe";
export const isVideoStep = (s: EffectStep) => VIDEO_EFFECTS.has(s.effect);
export interface EffectRecipe {
  version: 1;
  id: string;
  name: string;
  steps: EffectStep[];
}
export interface EffectCatalog {
  recipes: EffectRecipe[];
  warnings: string[];
}
export interface Clip {
  id: string;
  asset_id: string;
  position_ms: number;
  source_start_ms: number;
  source_end_ms: number;
  gain_milli: number;
  fade_in_ms: number;
  fade_out_ms: number;
  effects: EffectRecipe[];
  transform?: Transform | null;
  /** Transition from the previous main-track clip, centred on the cut. */
  transition_in?: Transition | null;
  /** Speed in thousandths (1000 = normal). */
  speed_milli?: number;
  reverse?: boolean;
  /** Overlay animation (replaces `transform` when present). */
  keyframes?: Keyframe[];
  /** Audio-track clips: dB to duck under voice (0 = off). */
  duck_db?: number;
}
export interface Track {
  id: string;
  name: string;
  kind: "video" | "audio" | "text";
  clips: Clip[];
}
export interface Settings {
  width: number;
  height: number;
  fps_num: number;
  fps_den: number;
  /** Crop to fill the frame instead of letterboxing. */
  fill?: boolean;
}
export type TransitionKind =
  | "fade"
  | "fadeblack"
  | "fadewhite"
  | "dissolve"
  | "wipeleft"
  | "wiperight"
  | "wipeup"
  | "wipedown"
  | "slideleft"
  | "slideright"
  | "slideup"
  | "slidedown"
  | "smoothleft"
  | "smoothright"
  | "circleopen"
  | "circleclose"
  | "radial"
  | "pixelize"
  | "zoomin"
  | "hblur";
export interface Transition {
  kind: TransitionKind;
  duration_ms: number;
}
export interface Project {
  version: number;
  id: string;
  name: string;
  settings: Settings;
  assets: Asset[];
  tracks: Track[];
}
export interface Snapshot {
  project: Project;
  revision: number;
}
export interface Proposal {
  id: string;
  revision: number;
  edits: Edit[];
  preview: Project;
}
export interface AgentConnection {
  config_path: string;
  allowed_root: string;
  /** "autonomous": the agent applies removals and large batches without review. */
  mode: "review" | "autonomous";
}
export type Edit =
  | { operation: "set_effects"; clip_id: string; effects: EffectRecipe[] }
  | { operation: "set_transform"; clip_id: string; transform: Transform | null }
  | { operation: "set_settings"; settings: Settings }
  | { operation: "set_ducking"; clip_id: string; duck_db: number }
  | { operation: "set_keyframes"; clip_id: string; keyframes: Keyframe[] }
  | {
      operation: "set_speed";
      clip_id: string;
      speed_milli: number;
      reverse: boolean;
    }
  | {
      operation: "set_transition";
      clip_id: string;
      transition: Transition | null;
      make_room?: boolean;
    }
  | {
      operation: "add_track";
      track_id: string;
      name: string;
      kind: "video" | "audio";
    }
  | {
      operation: "move_clip";
      clip_id: string;
      track_id: string;
      position_ms: number;
    }
  | {
      operation: "set_audio";
      clip_id: string;
      gain_milli: number;
      fade_in_ms: number;
      fade_out_ms: number;
    }
  | { operation: "append"; asset_id: string; clip_id: string; track_id: string }
  | {
      operation: "split_clip";
      clip_id: string;
      position_ms: number;
      new_clip_id: string;
    }
  | {
      operation: "trim_clip";
      clip_id: string;
      start_ms: number;
      end_ms: number;
    }
  | { operation: "delete_clip"; clip_id: string }
  | { operation: "reorder_clip"; clip_id: string; index: number };
export const time = (ms: number) => {
  const v = Math.max(0, Math.round(ms));
  return `${Math.floor(v / 60000)
    .toString()
    .padStart(2, "0")}:${Math.floor((v / 1000) % 60)
    .toString()
    .padStart(2, "0")}.${(v % 1000).toString().padStart(3, "0")}`;
};
