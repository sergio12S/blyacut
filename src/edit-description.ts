import { time, type Edit, type Project } from './types';

/** Human review text; editing and validation remain in the Rust core. */
export function describeEdit(edit: Edit, project: Project | null | undefined): string {
  const track = (id: string) => project?.tracks.find(t => t.id === id)?.name ?? id;
  const asset = (id: string) => project?.assets.find(a => a.id === id)?.name ?? id;
  const clip = (id: string) => {
    const found = project?.tracks.flatMap(t => t.clips).find(c => c.id === id);
    return found ? asset(found.asset_id) : id;
  };
  switch (edit.operation) {
    case 'add_track': return `Новая ${edit.kind === 'audio' ? 'звуковая' : 'видео'}дорожка «${edit.name}»`;
    case 'append': return `Добавить «${asset(edit.asset_id)}» на ${track(edit.track_id)}`;
    case 'split_clip': return `Разрезать «${clip(edit.clip_id)}» на ${time(edit.position_ms)}`;
    case 'trim_clip': return `Обрезать «${clip(edit.clip_id)}» до ${time(edit.start_ms)}–${time(edit.end_ms)} исходника`;
    case 'delete_clip': return `Удалить «${clip(edit.clip_id)}»`;
    case 'move_clip': return `Переместить «${clip(edit.clip_id)}» на ${track(edit.track_id)}, ${time(edit.position_ms)}`;
    case 'reorder_clip': return `Переставить «${clip(edit.clip_id)}» на позицию ${edit.index + 1}`;
    case 'set_audio': return `Звук «${clip(edit.clip_id)}»: ${edit.gain_milli / 10} %, появление ${time(edit.fade_in_ms)}, затухание ${time(edit.fade_out_ms)}`;
    case 'set_effects': return `Эффекты «${clip(edit.clip_id)}»: ${edit.effects.map(f => f.name).join(' → ') || 'убрать все'}`;
    default: { const exhaustive: never = edit; return exhaustive; }
  }
}
