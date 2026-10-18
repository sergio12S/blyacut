import type { Clip, Edit, Project } from "./types";

export type GestureMode = "move" | "trim-start" | "trim-end";
/** Include the border itself in a comfortable 10px resize target. */
export function clipHitMode(offset: number, width: number): GestureMode {
  const edge = Math.min(10, width / 4);
  return offset <= edge
    ? "trim-start"
    : offset >= width - edge
      ? "trim-end"
      : "move";
}
export interface Placement {
  trackId: string;
  position: number;
  start: number;
  end: number;
}
export interface GesturePlan {
  edits: Edit[];
  placements: Record<string, Placement>;
  guide?: number;
  error?: string;
  label: "Move" | "Insert" | "Ripple trim" | "Trim";
}
const MAX_TIME = 604800000;
export const clipLength = (clip: Clip) =>
  clip.source_end_ms - clip.source_start_ms;
export function frameTime(ms: number, settings: Project["settings"]): number {
  return Math.round(
    (Math.round((ms * settings.fps_num) / (1000 * settings.fps_den)) *
      1000 *
      settings.fps_den) /
      settings.fps_num,
  );
}
/** Snap either moving edge; ties prefer the start. Threshold is screen-space. */
export function snapPosition(
  position: number,
  length: number,
  targets: number[],
  threshold: number,
) {
  let delta = threshold + 1;
  let guide: number | undefined;
  for (const edge of [position, position + length]) {
    for (const target of targets) {
      const distance = target - edge;
      if (
        Math.abs(distance) <= threshold &&
        Math.abs(distance) < Math.abs(delta) &&
        position + distance >= 0
      ) {
        delta = distance;
        guide = target;
      }
    }
  }
  return { position: guide === undefined ? position : position + delta, guide };
}
/** Produces only command intents and transient geometry. Rust validates/commits them. */
export function planGesture(
  project: Project,
  clipId: string,
  mode: GestureMode,
  delta: number,
  targetId: string,
  options: {
    magnetic: boolean;
    snap: boolean;
    pixelsPerSecond: number;
    playhead: number;
  },
): GesturePlan {
  const source = project.tracks.find((t) =>
    t.clips.some((c) => c.id === clipId),
  );
  const clip = source?.clips.find((c) => c.id === clipId);
  const target = project.tracks.find((t) => t.id === targetId);
  const plan: GesturePlan = { edits: [], placements: {}, label: "Move" };
  if (
    !source ||
    !clip ||
    !target ||
    source.kind !== target.kind ||
    target.kind === "text"
  ) {
    return { ...plan, error: "Drop on a track of the same type" };
  }
  const original = {
    trackId: source.id,
    position: clip.position_ms,
    start: clip.source_start_ms,
    end: clip.source_end_ms,
  };
  const next = { ...original };
  plan.placements[clipId] = next;
  const targets = [
    0,
    options.playhead,
    ...project.tracks.flatMap((t) =>
      t.clips
        .filter((c) => c.id !== clipId)
        .flatMap((c) => [c.position_ms, c.position_ms + clipLength(c)]),
    ),
  ];
  const snap = (position: number, length: number) => {
    const result = options.snap
      ? snapPosition(position, length, targets, 8000 / options.pixelsPerSecond)
      : { position, guide: undefined };
    plan.guide = result.guide;
    return Math.round(result.position);
  };
  if (mode === "move") {
    if (
      options.magnetic &&
      source.id === target.id &&
      source.kind === "video"
    ) {
      plan.label = "Insert";
      const remaining = source.clips.filter((c) => c.id !== clipId);
      const center = clip.position_ms + delta + clipLength(clip) / 2;
      const index = remaining.filter(
        (c) => center > c.position_ms + clipLength(c) / 2,
      ).length;
      const order = [...remaining];
      order.splice(index, 0, clip);
      let cursor = 0;
      for (const c of order) {
        plan.placements[c.id] = {
          trackId: source.id,
          position: cursor,
          start: c.source_start_ms,
          end: c.source_end_ms,
        };
        cursor += clipLength(c);
      }
      if (index !== source.clips.indexOf(clip))
        plan.edits = [{ operation: "reorder_clip", clip_id: clip.id, index }];
      return plan;
    }
    next.position = snap(
      Math.max(0, frameTime(clip.position_ms + delta, project.settings)),
      clipLength(clip),
    );
    next.trackId = target.id;
    if (next.position !== clip.position_ms || target.id !== source.id)
      plan.edits = [
        {
          operation: "move_clip",
          clip_id: clip.id,
          track_id: target.id,
          position_ms: next.position,
        },
      ];
  } else {
    plan.label = source.kind === "video" ? "Ripple trim" : "Trim";
    const asset = project.assets.find((a) => a.id === clip.asset_id)!;
    const minLength = Math.max(
      1,
      Math.ceil((1000 * project.settings.fps_den) / project.settings.fps_num),
      clip.fade_in_ms + clip.fade_out_ms,
    );
    if (mode === "trim-start") {
      let change = frameTime(delta, project.settings);
      change =
        snap(
          source.kind === "audio"
            ? Math.max(0, clip.position_ms + change)
            : clip.position_ms + change,
          0,
        ) - clip.position_ms;
      change = Math.max(
        -clip.source_start_ms,
        source.kind === "audio" ? -clip.position_ms : -clip.source_start_ms,
        Math.min(clipLength(clip) - minLength, change),
      );
      next.start += change;
      if (source.kind === "audio") next.position += change;
    } else {
      const edge = snap(
        Math.max(
          0,
          frameTime(
            clip.position_ms + clipLength(clip) + delta,
            project.settings,
          ),
        ),
        0,
      );
      next.end = Math.max(
        clip.source_start_ms + minLength,
        Math.min(
          asset.duration_ms,
          clip.source_start_ms + edge - clip.position_ms,
        ),
      );
    }
    if (next.start !== original.start || next.end !== original.end) {
      const trim: Edit = {
        operation: "trim_clip",
        clip_id: clip.id,
        start_ms: next.start,
        end_ms: next.end,
      };
      const move: Edit = {
        operation: "move_clip",
        clip_id: clip.id,
        track_id: source.id,
        position_ms: next.position,
      };
      plan.edits =
        next.position === original.position
          ? [trim]
          : next.position < original.position
            ? [move, trim]
            : [trim, move];
    }
    if (source.kind === "video") {
      let cursor = 0;
      for (const c of source.clips) {
        const placement =
          c.id === clipId
            ? next
            : {
                trackId: source.id,
                position: c.position_ms,
                start: c.source_start_ms,
                end: c.source_end_ms,
              };
        placement.position = cursor;
        plan.placements[c.id] = placement;
        cursor += placement.end - placement.start;
      }
      // The left edge is consumed by ripple; do not draw a misleading snap guide.
      if (mode === "trim-start") plan.guide = undefined;
    }
  }
  const length = next.end - next.start;
  if (next.position + length > MAX_TIME)
    plan.error = "Timeline is limited to one week";
  if (
    target.clips.some(
      (c) =>
        c.id !== clipId &&
        next.position < c.position_ms + clipLength(c) &&
        next.position + length > c.position_ms,
    ) &&
    !(mode !== "move" && source.kind === "video")
  ) {
    plan.error =
      "Clips cannot overlap here — use another track or enable Magnetic for video";
  }
  if (
    plan.guide !== undefined &&
    ![next.position, next.position + length].includes(plan.guide)
  )
    plan.guide = undefined;
  return plan;
}
