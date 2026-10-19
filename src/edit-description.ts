import { time, type Edit, type Project } from "./types";
import { TRANSITION_NAMES } from "./transitions";

/** Human review text; editing and validation remain in the Rust core. */
export function describeEdit(
  edit: Edit,
  project: Project | null | undefined,
): string {
  const track = (id: string) =>
    project?.tracks.find((t) => t.id === id)?.name ?? id;
  const asset = (id: string) =>
    project?.assets.find((a) => a.id === id)?.name ?? id;
  const clip = (id: string) => {
    const found = project?.tracks
      .flatMap((t) => t.clips)
      .find((c) => c.id === id);
    return found ? asset(found.asset_id) : id;
  };
  switch (edit.operation) {
    case "add_track":
      return `New ${edit.kind} track “${edit.name}”`;
    case "append":
      return `Add “${asset(edit.asset_id)}” to ${track(edit.track_id)}`;
    case "split_clip":
      return `Split “${clip(edit.clip_id)}” at ${time(edit.position_ms)}`;
    case "trim_clip":
      return `Trim “${clip(edit.clip_id)}” to source ${time(edit.start_ms)}–${time(edit.end_ms)}`;
    case "delete_clip":
      return `Remove “${clip(edit.clip_id)}”`;
    case "move_clip":
      return `Move “${clip(edit.clip_id)}” to ${track(edit.track_id)} at ${time(edit.position_ms)}`;
    case "reorder_clip":
      return `Reorder “${clip(edit.clip_id)}” to position ${edit.index + 1}`;
    case "set_audio":
      return `Sound of “${clip(edit.clip_id)}”: ${edit.gain_milli / 10}%, fade in ${time(edit.fade_in_ms)}, fade out ${time(edit.fade_out_ms)}`;
    case "set_effects":
      return `Effects on “${clip(edit.clip_id)}”: ${edit.effects.map((f) => f.name).join(" → ") || "remove all"}`;
    case "set_settings": {
      const s = edit.settings;
      return `Format ${s.width}×${s.height}, ${Math.round((s.fps_num / s.fps_den) * 100) / 100} fps${s.fill ? ", crop to fill" : ""}`;
    }
    case "set_keyframes":
      return edit.keyframes.length
        ? `Animate “${clip(edit.clip_id)}”: ${edit.keyframes.length} keyframes`
        : `Remove animation from “${clip(edit.clip_id)}”`;
    case "set_ducking":
      return edit.duck_db
        ? `Duck “${clip(edit.clip_id)}” by ${edit.duck_db} dB under voice`
        : `No ducking for “${clip(edit.clip_id)}”`;
    case "set_speed":
      return `Speed of “${clip(edit.clip_id)}”: ${edit.speed_milli / 1000}×${edit.reverse ? ", reversed" : ""}`;
    case "set_transition":
      return edit.transition
        ? `${TRANSITION_NAMES[edit.transition.kind]} transition (${edit.transition.duration_ms / 1000} s) into “${clip(edit.clip_id)}”`
        : `Remove the transition into “${clip(edit.clip_id)}”`;
    case "set_transform": {
      const t = edit.transform;
      return t
        ? `Place “${clip(edit.clip_id)}”: centre ${t.x_milli / 10}% × ${t.y_milli / 10}%, scale ${t.scale_milli / 10}%, opacity ${t.opacity_milli / 10}%`
        : `“${clip(edit.clip_id)}” full frame`;
    }
    default: {
      const exhaustive: never = edit;
      return exhaustive;
    }
  }
}
