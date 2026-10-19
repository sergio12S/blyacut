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
      return `Новая ${edit.kind === "audio" ? "звуковая" : "видео"}дорожка «${edit.name}»`;
    case "append":
      return `Добавить «${asset(edit.asset_id)}» на ${track(edit.track_id)}`;
    case "split_clip":
      return `Разрезать «${clip(edit.clip_id)}» на ${time(edit.position_ms)}`;
    case "trim_clip":
      return `Обрезать «${clip(edit.clip_id)}» до ${time(edit.start_ms)}–${time(edit.end_ms)} исходника`;
    case "delete_clip":
      return `Удалить «${clip(edit.clip_id)}»`;
    case "move_clip":
      return `Переместить «${clip(edit.clip_id)}» на ${track(edit.track_id)}, ${time(edit.position_ms)}`;
    case "reorder_clip":
      return `Переставить «${clip(edit.clip_id)}» на позицию ${edit.index + 1}`;
    case "set_audio":
      return `Звук «${clip(edit.clip_id)}»: ${edit.gain_milli / 10} %, появление ${time(edit.fade_in_ms)}, затухание ${time(edit.fade_out_ms)}`;
    case "set_effects":
      return `Эффекты «${clip(edit.clip_id)}»: ${edit.effects.map((f) => f.name).join(" → ") || "убрать все"}`;
    case "set_settings": {
      const s = edit.settings;
      return `Формат ${s.width}×${s.height}, ${Math.round((s.fps_num / s.fps_den) * 100) / 100} к/с${s.fill ? ", заполнить кадр" : ""}`;
    }
    case "set_keyframes":
      return edit.keyframes.length
        ? `Анимация «${clip(edit.clip_id)}»: ${edit.keyframes.length} ключ.`
        : `Без анимации: «${clip(edit.clip_id)}»`;
    case "set_ducking":
      return edit.duck_db
        ? `«${clip(edit.clip_id)}» приглушается на ${edit.duck_db} дБ под голос`
        : `Без приглушения: «${clip(edit.clip_id)}»`;
    case "set_speed":
      return `Скорость «${clip(edit.clip_id)}»: ${edit.speed_milli / 1000}×${edit.reverse ? ", в обратную сторону" : ""}`;
    case "set_transition":
      return edit.transition
        ? `Переход «${TRANSITION_NAMES[edit.transition.kind]}» ${edit.transition.duration_ms / 1000} с перед «${clip(edit.clip_id)}»`
        : `Убрать переход перед «${clip(edit.clip_id)}»`;
    case "set_transform": {
      const t = edit.transform;
      return t
        ? `Положение «${clip(edit.clip_id)}»: центр ${t.x_milli / 10} % × ${t.y_milli / 10} %, масштаб ${t.scale_milli / 10} %, непрозрачность ${t.opacity_milli / 10} %`
        : `«${clip(edit.clip_id)}» — во весь кадр`;
    }
    default: {
      const exhaustive: never = edit;
      return exhaustive;
    }
  }
}
