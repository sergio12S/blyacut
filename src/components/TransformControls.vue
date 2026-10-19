<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useEditor } from "../stores/editor";
import { clipLength } from "../timeline-gesture";
import {
  FULL_FRAME,
  time,
  transformAt,
  type Keyframe,
  type Transform,
} from "../types";
/** Position, scale and opacity of an overlay clip (animated with keyframes); each release
 * commits one Undo step. */
const e = useEditor();
const t = ref<Transform>({ ...FULL_FRAME });
const keys = computed(() => e.selection?.keyframes ?? []);
const local = computed(() =>
  e.selection
    ? Math.min(
        Math.max(0, Math.round(e.playhead - e.selection.position_ms)),
        clipLength(e.selection),
      )
    : 0,
);
watch(
  () => [e.selection, local.value] as const,
  ([clip]) =>
    (t.value = { ...(clip ? transformAt(clip, local.value) : FULL_FRAME) }),
  { immediate: true, deep: true },
);
function setKeys(keyframes: Keyframe[]) {
  if (!e.selection) return;
  void e.edit({
    operation: "set_keyframes",
    clip_id: e.selection.id,
    keyframes: [...keyframes].sort((a, b) => a.at_ms - b.at_ms),
  });
}
/** Add or replace the key under the playhead with the slider values. */
function keyHere() {
  const c = e.selection;
  if (!c) return;
  const others = keys.value.filter((k) => k.at_ms !== local.value);
  const base =
    !keys.value.length && local.value > 0
      ? [{ at_ms: 0, transform: c.transform ?? FULL_FRAME }]
      : [];
  setKeys([
    ...base,
    ...others,
    { at_ms: local.value, transform: { ...t.value } },
  ]);
}
// Entrance/motion presets ending (or starting) at the clip's current placement.
function animate(kind: "left" | "bottom" | "fade" | "zoom") {
  const c = e.selection;
  if (!c) return;
  const pose = {
    ...(keys.value.length
      ? transformAt(c, clipLength(c))
      : (c.transform ?? FULL_FRAME)),
  };
  const duration = clipLength(c);
  const ramp = Math.min(600, duration);
  const from: Record<typeof kind, Transform> = {
    left: { ...pose, x_milli: -300 },
    bottom: { ...pose, y_milli: 1300 },
    fade: { ...pose, opacity_milli: 0 },
    zoom: pose,
  };
  if (kind === "zoom")
    setKeys([
      { at_ms: 0, transform: pose },
      {
        at_ms: duration,
        transform: {
          ...pose,
          scale_milli: Math.min(4000, Math.round(pose.scale_milli * 1.25)),
        },
      },
    ]);
  else
    setKeys([
      { at_ms: 0, transform: from[kind] },
      { at_ms: ramp, transform: pose },
    ]);
}
function draft() {
  if (e.selection)
    e.draftTransform = { clipId: e.selection.id, transform: { ...t.value } };
}
function commit(transform: Transform | null) {
  e.draftTransform = undefined;
  if (!e.selection) return;
  // Animated clips: sliders and placement presets edit the key under the playhead.
  if (keys.value.length && transform) {
    t.value = { ...transform };
    return keyHere();
  }
  void e.editBatch([
    ...(keys.value.length
      ? [
          {
            operation: "set_keyframes" as const,
            clip_id: e.selection.id,
            keyframes: [],
          },
        ]
      : []),
    { operation: "set_transform", clip_id: e.selection.id, transform },
  ]);
}
const presets: [string, Transform | null][] = [
  ["Во весь кадр", null],
  [
    "Картинка в картинке ↘",
    { x_milli: 800, y_milli: 780, scale_milli: 300, opacity_milli: 1000 },
  ],
  ["↗", { x_milli: 800, y_milli: 220, scale_milli: 300, opacity_milli: 1000 }],
  ["↖", { x_milli: 200, y_milli: 220, scale_milli: 300, opacity_milli: 1000 }],
  [
    "Центр 50 %",
    { x_milli: 500, y_milli: 500, scale_milli: 500, opacity_milli: 1000 },
  ],
];
const sliders: [
  keyof Transform,
  string,
  number,
  number,
  (v: number) => string,
][] = [
  ["x_milli", "Положение X", -300, 1300, (v) => `${v / 10} %`],
  ["y_milli", "Положение Y", -300, 1300, (v) => `${v / 10} %`],
  ["scale_milli", "Масштаб", 10, 2000, (v) => `${v / 10} %`],
  ["opacity_milli", "Непрозрачность", 0, 1000, (v) => `${v / 10} %`],
];
</script>
<template>
  <section class="transform" aria-label="Положение наложения">
    <h4>Наложение</h4>
    <div class="presets">
      <button
        v-for="[name, value] in presets"
        :key="name"
        :title="name"
        @click="commit(value)"
      >
        {{ name }}
      </button>
    </div>
    <h4>Анимация</h4>
    <div class="presets">
      <button title="Въезжает слева за 0,6 с" @click="animate('left')">
        ⇢ Выезд слева
      </button>
      <button title="Въезжает снизу за 0,6 с" @click="animate('bottom')">
        ⇡ Выезд снизу
      </button>
      <button title="Проявляется из прозрачного" @click="animate('fade')">
        ◐ Проявление
      </button>
      <button
        title="Медленно увеличивается до конца клипа"
        @click="animate('zoom')"
      >
        ⊕ Наезд
      </button>
      <button
        title="Ключевой кадр в позиции плейхеда с текущими значениями"
        @click="keyHere"
      >
        ◆ Ключ здесь
      </button>
      <button v-if="keys.length" class="danger" @click="setKeys([])">
        Убрать анимацию
      </button>
    </div>
    <div v-if="keys.length" class="keys" aria-label="Ключевые кадры">
      <span
        v-for="k in keys"
        :key="k.at_ms"
        class="key"
        :class="{ here: k.at_ms === local }"
      >
        <button
          class="ghost"
          :title="`Перейти к ключу ${time(k.at_ms)}`"
          @click="e.seek(e.selection!.position_ms + k.at_ms)"
        >
          ◆ {{ time(k.at_ms).slice(3) }}</button
        ><button
          class="ghost"
          aria-label="Удалить ключ"
          @click="setKeys(keys.filter((x) => x.at_ms !== k.at_ms))"
        >
          ×
        </button>
      </span>
    </div>
    <label
      v-for="[key, label, min, max, show] in sliders"
      :key="key"
      class="slider"
    >
      <span>{{ label }}</span>
      <input
        v-model.number="t[key]"
        type="range"
        :min="min"
        :max="max"
        step="5"
        :aria-label="label"
        @input="draft"
        @change="commit(t)"
      /><output>{{ show(t[key]) }}</output>
    </label>
  </section>
</template>
<style scoped>
.transform {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 10px;
  border-top: 1px solid var(--border-subtle);
}
h4 {
  margin: 0;
  font-size: 12px;
  color: var(--text-secondary);
  font-weight: 600;
}
.presets {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.presets button {
  padding: 3px 8px;
  font-size: 11px;
}
.keys {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.key {
  display: inline-flex;
  border: 1px solid var(--border-control);
  border-radius: 999px;
  font: 11px var(--font-code);
}
.key.here {
  border-color: var(--accent);
  color: var(--accent);
}
.key button {
  padding: 2px 6px;
  background: transparent;
  border: 0;
  font: inherit;
  color: inherit;
}
.slider {
  display: grid;
  grid-template-columns: 96px minmax(0, 1fr) 52px;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: var(--text-secondary);
}
.slider input {
  accent-color: var(--accent);
  width: 100%;
}
.slider output {
  font: 11px var(--font-code);
  color: var(--text-primary);
  text-align: right;
}
</style>
