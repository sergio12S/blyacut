<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, shallowRef, watch } from "vue";
import { useEditor } from "../stores/editor";
import { RECIPE_MIME, time, trackName, type Clip } from "../types";
import {
  clipLength,
  clipHitMode,
  planGesture,
  type GestureMode,
  type GesturePlan,
} from "../timeline-gesture";
const e = useEditor();
const zoom = ref(65);
const magnetic = ref(true);
const snapping = ref(true);
const ruler = ref<HTMLElement>();
const scroller = ref<HTMLElement>();
const tracksScroller = ref<HTMLElement>();
const notice = ref("");
const labels: Record<GesturePlan["label"], string> = {
  Move: "Перемещение",
  Insert: "Вставка",
  "Ripple trim": "Обрезка со сдвигом",
  Trim: "Обрезка",
};
const dropTarget = ref("");
function acceptRecipe(event: DragEvent, clipId: string) {
  if (!event.dataTransfer?.types.includes(RECIPE_MIME)) return;
  event.preventDefault();
  event.dataTransfer.dropEffect = "copy";
  dropTarget.value = clipId;
}
function dropRecipe(event: DragEvent, clipId: string) {
  dropTarget.value = "";
  const raw = event.dataTransfer?.getData(RECIPE_MIME);
  if (raw) void e.applyRecipe(clipId, JSON.parse(raw));
}
interface Gesture {
  pointerId: number;
  element: HTMLElement;
  clip: Clip;
  sourceId: string;
  mode: GestureMode;
  startX: number;
  startY: number;
  startTime: number;
  x: number;
  y: number;
  unsnapped: boolean;
  revision: number;
  moved: boolean;
}
const gesture = shallowRef<Gesture>();
const preview = ref<GesturePlan>();
let animation = 0;
const ghost = computed(
  () => gesture.value && preview.value?.placements[gesture.value.clip.id],
);
const width = computed(() =>
  Math.max(
    900,
    (Math.max(
      e.duration,
      ghost.value
        ? ghost.value.position + ghost.value.end - ghost.value.start
        : 0,
    ) /
      1000) *
      zoom.value +
      160,
  ),
);
const ticks = computed(() =>
  Array.from(
    { length: Math.min(2000, Math.ceil(width.value / (zoom.value * 5))) },
    (_, i) => i * 5,
  ),
);
function timelineTime(x: number) {
  return (
    ((x - (ruler.value?.getBoundingClientRect().left ?? x)) / zoom.value) * 1000
  );
}
function seek(event: MouseEvent) {
  if (gesture.value) return;
  e.playing = false;
  e.seek(timelineTime(event.clientX));
}
function begin(event: PointerEvent, clip: Clip, sourceId: string) {
  if (event.button !== 0 || !event.isPrimary || e.busy || gesture.value) return;
  event.preventDefault();
  const element = event.currentTarget as HTMLElement;
  const bounds = element.getBoundingClientRect();
  const mode = clipHitMode(event.clientX - bounds.left, bounds.width);
  e.playing = false;
  e.selected = clip.id;
  element.focus({ preventScroll: true });
  element.setPointerCapture(event.pointerId);
  gesture.value = {
    pointerId: event.pointerId,
    element,
    clip,
    sourceId,
    mode,
    startX: event.clientX,
    startY: event.clientY,
    startTime: timelineTime(event.clientX),
    x: event.clientX,
    y: event.clientY,
    unsnapped: event.metaKey || event.ctrlKey,
    revision: e.revision,
    moved: false,
  };
  notice.value = "";
  animation = requestAnimationFrame(autoScroll);
}
function updatePreview() {
  const g = gesture.value;
  if (!g || !e.project) return;
  let targetId = g.mode === "move" ? "" : g.sourceId;
  const viewport = tracksScroller.value?.getBoundingClientRect();
  if (
    g.mode === "move" &&
    viewport &&
    g.y >= viewport.top &&
    g.y < viewport.bottom
  ) {
    for (const lane of scroller.value?.querySelectorAll<HTMLElement>(
      "[data-track-id]",
    ) ?? []) {
      const rect = lane.getBoundingClientRect();
      if (g.y >= rect.top && g.y < rect.bottom) {
        targetId = lane.dataset.trackId!;
        break;
      }
    }
  }
  preview.value = planGesture(
    e.project,
    g.clip.id,
    g.mode,
    timelineTime(g.x) - g.startTime,
    targetId,
    {
      magnetic: magnetic.value,
      snap: snapping.value && !g.unsnapped,
      pixelsPerSecond: zoom.value,
      playhead: e.playhead,
    },
  );
}
function move(event: PointerEvent) {
  const g = gesture.value;
  if (!g || event.pointerId !== g.pointerId) return;
  g.x = event.clientX;
  g.y = event.clientY;
  g.unsnapped = event.metaKey || event.ctrlKey;
  if (!g.moved && Math.hypot(g.x - g.startX, g.y - g.startY) < 4) return;
  g.moved = true;
  updatePreview();
}
function autoScroll() {
  const g = gesture.value;
  if (!g) return;
  if (g.moved && scroller.value && tracksScroller.value) {
    const horizontal = scroller.value.getBoundingClientRect();
    const vertical = tracksScroller.value.getBoundingClientRect();
    const speed = (p: number, min: number, max: number) =>
      p < min + 32
        ? -Math.min(14, (min + 32 - p) / 3)
        : p > max - 32
          ? Math.min(14, (p - max + 32) / 3)
          : 0;
    const oldX = scroller.value.scrollLeft;
    const oldY = tracksScroller.value.scrollTop;
    scroller.value.scrollLeft += speed(g.x, horizontal.left, horizontal.right);
    tracksScroller.value.scrollTop += speed(g.y, vertical.top, vertical.bottom);
    if (
      oldX !== scroller.value.scrollLeft ||
      oldY !== tracksScroller.value.scrollTop
    )
      updatePreview();
  }
  animation = requestAnimationFrame(autoScroll);
}
function clearGesture() {
  const g = gesture.value;
  gesture.value = undefined;
  preview.value = undefined;
  cancelAnimationFrame(animation);
  if (g?.element.hasPointerCapture(g.pointerId))
    g.element.releasePointerCapture(g.pointerId);
}
function finish(event: PointerEvent) {
  const g = gesture.value;
  if (!g || g.pointerId !== event.pointerId) return;
  move(event);
  const plan = preview.value;
  clearGesture();
  if (!g.moved) {
    e.select(g.clip);
    return;
  }
  if (e.busy) {
    notice.value = "Редактор занят — начните перетаскивание заново";
    return;
  }
  if (plan?.error) {
    notice.value = plan.error;
    return;
  }
  if (plan?.edits.length) void e.editBatch(plan.edits, g.revision);
}
function cancel() {
  if (gesture.value) {
    clearGesture();
    notice.value = "Перетаскивание отменено";
  }
}
function keyboard(event: KeyboardEvent) {
  if (gesture.value && event.key === "Escape") {
    event.preventDefault();
    event.stopImmediatePropagation();
    cancel();
  }
}
watch(
  () => e.revision,
  () => {
    if (gesture.value) {
      clearGesture();
      notice.value = "Проект изменился — начните заново";
    }
  },
);
onMounted(() => window.addEventListener("keydown", keyboard, true));
onUnmounted(() => {
  clearGesture();
  window.removeEventListener("keydown", keyboard, true);
});
function position(c: Clip) {
  return c.id !== gesture.value?.clip.id
    ? (preview.value?.placements[c.id]?.position ?? c.position_ms)
    : c.position_ms;
}
</script>
<template>
  <section
    class="timeline"
    :class="{ 'is-dragging': gesture?.moved }"
    @pointermove="move"
    @pointerup="finish"
    @pointercancel="cancel"
    @lostpointercapture="cancel"
  >
    <div class="timeline-toolbar">
      <div class="toolbar-group">
        <button
          :disabled="!e.duration"
          title="Разрезать в плейхеде (S)"
          @click="e.split()"
        >
          ✂ Разрезать</button
        ><button
          :disabled="!e.selection"
          title="Удалить (⌫)"
          @click="e.remove()"
        >
          Удалить</button
        ><button title="Добавить звуковую дорожку" @click="e.addAudioTrack()">
          ＋ Дорожка
        </button>
        <button
          class="mode-toggle"
          :aria-pressed="magnetic"
          :disabled="!!gesture"
          title="Видео склеивается без зазоров. Выключите, чтобы двигать клипы свободно."
          @click="magnetic = !magnetic"
        >
          Магнит
        </button>
        <button
          class="mode-toggle snap-toggle"
          :aria-pressed="snapping"
          :disabled="!!gesture"
          title="Прилипание к краям клипов и плейхеду. ⌘ / Ctrl — временно отключить."
          @click="snapping = !snapping"
        >
          Прилипание
        </button>
      </div>
      <label class="zoom"
        >−
        <input
          v-model.number="zoom"
          type="range"
          min="15"
          max="160"
          aria-label="Масштаб таймлайна"
          :disabled="!!gesture"
        />
        +</label
      >
    </div>
    <div ref="tracksScroller" class="tracks-scroll">
      <div class="timeline-body">
        <div class="track-label">
          <div class="ruler-spacer" />
          <div
            v-for="(track, index) in e.project?.tracks"
            :key="track.id"
            class="track-label-row"
          >
            <span class="track-icon"
              >{{ track.kind === "audio" ? "A" : "V"
              }}{{
                e.project?.tracks
                  .slice(0, index + 1)
                  .filter((t) => t.kind === track.kind).length
              }}</span
            ><span
              >{{ trackName(track.name)
              }}<small>{{
                track.kind === "audio" ? "Звук" : "Видео + звук"
              }}</small></span
            >
          </div>
        </div>
        <div ref="scroller" class="timeline-scroll">
          <div :style="{ width: width + 'px' }" class="timeline-content">
            <div ref="ruler" class="ruler" @pointerdown="seek">
              <span
                v-for="t in ticks"
                :key="t"
                :style="{ left: t * zoom + 'px' }"
                >{{ time(t * 1000).slice(0, 5) }}</span
              >
            </div>
            <div
              v-for="track in e.project?.tracks"
              :key="track.id"
              :data-track-id="track.id"
              class="track-lane"
              :class="{ audio: track.kind === 'audio' }"
              @pointerdown.self="seek"
            >
              <button
                v-for="c in track.clips"
                :key="c.id"
                class="clip"
                :class="{
                  selected: e.selected === c.id,
                  audio: track.kind === 'audio',
                  'drag-origin': gesture?.moved && gesture.clip.id === c.id,
                  'drop-target': dropTarget === c.id,
                }"
                :style="{
                  left: (position(c) / 1000) * zoom + 'px',
                  width:
                    ((c.source_end_ms - c.source_start_ms) / 1000) * zoom +
                    'px',
                }"
                :data-clip-id="c.id"
                :aria-label="`${e.project?.assets.find((a) => a.id === c.asset_id)?.name}, ${track.name}, ${time(c.position_ms)}, длительность ${time(clipLength(c))}`"
                @pointerdown.stop="begin($event, c, track.id)"
                @click.stop.prevent="
                  (event) => {
                    if (event.detail === 0) e.select(c);
                  }
                "
                @contextmenu.prevent="e.select(c)"
                @dragover="acceptRecipe($event, c.id)"
                @dragleave="dropTarget = ''"
                @drop.prevent="dropRecipe($event, c.id)"
              >
                <span
                  class="trim-handle start"
                  data-edge="start"
                  title="Тяните, чтобы обрезать начало"
                />
                <span
                  class="trim-handle end"
                  data-edge="end"
                  title="Тяните, чтобы обрезать конец"
                />
                <span class="clip-label">{{
                  e.project?.assets.find((a) => a.id === c.asset_id)?.name
                }}</span
                ><span class="clip-duration"
                  >{{ time(c.source_end_ms - c.source_start_ms) }} ·
                  {{ c.gain_milli / 10 }}%</span
                >
                <div class="clip-stripes" />
              </button>
              <div
                v-if="gesture?.moved && ghost?.trackId === track.id"
                class="clip drag-ghost"
                :class="{
                  invalid: preview?.error,
                  audio: track.kind === 'audio',
                }"
                :style="{
                  left: (ghost.position / 1000) * zoom + 'px',
                  width: ((ghost.end - ghost.start) / 1000) * zoom + 'px',
                }"
              >
                <span class="clip-label"
                  >{{ preview?.label }} · {{ time(ghost.position) }}</span
                >
                <span class="clip-duration">{{
                  time(ghost.end - ghost.start)
                }}</span>
              </div>
              <span v-if="!track.clips.length" class="timeline-hint">{{
                track.kind === "audio"
                  ? "Перетащите сюда музыку или голос из библиотеки"
                  : "Добавьте видео или титр из библиотеки"
              }}</span>
            </div>
            <div
              v-if="gesture?.moved && preview?.guide !== undefined"
              class="snap-guide"
              :style="{ left: (preview.guide / 1000) * zoom + 'px' }"
            />
            <div
              class="playhead"
              :style="{ left: (e.playhead / 1000) * zoom + 'px' }"
            >
              <i />
            </div>
          </div>
        </div>
      </div>
    </div>
    <div
      v-if="gesture?.moved || preview?.error || notice"
      class="timeline-status"
      role="status"
      :class="{ 'drag-warning': preview?.error || notice }"
    >
      {{
        preview?.error ||
        notice ||
        `${labels[preview?.label ?? "Move"]} · отпустите, чтобы применить · Esc — отмена · ⌘ — без прилипания`
      }}
    </div>
  </section>
</template>
