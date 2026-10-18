export interface Asset {
  id: string;
  name: string;
  path: string;
  duration_ms: number;
  width: number;
  height: number;
  has_audio: boolean;
}
export type EffectStep =
  | { effect: "brightness" | "contrast" | "saturation"; gain_milli: number }
  | { effect: "hue"; degrees: number }
  | { effect: "low_pass" | "high_pass"; frequency_hz: number }
  | { effect: "echo"; delay_ms: number; decay_milli: number };
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
}
export interface Track {
  id: string;
  name: string;
  kind: "video" | "audio" | "text";
  clips: Clip[];
}
export interface Project {
  version: number;
  id: string;
  name: string;
  settings: { width: number; height: number; fps_num: number; fps_den: number };
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
}
export type Edit =
  | { operation: "set_effects"; clip_id: string; effects: EffectRecipe[] }
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
