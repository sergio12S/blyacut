<script setup lang="ts">
import { ref, watch } from "vue";
import { useEditor } from "../stores/editor";
import { FULL_FRAME, type Transform } from "../types";
/** Position, scale and opacity of an overlay clip; each release commits one Undo step. */
const e = useEditor();
const t = ref<Transform>({ ...FULL_FRAME });
watch(
  () => e.selection?.transform,
  (value) => (t.value = { ...(value ?? FULL_FRAME) }),
  { immediate: true },
);
function draft() {
  if (e.selection)
    e.draftTransform = { clipId: e.selection.id, transform: { ...t.value } };
}
function commit(transform: Transform | null) {
  e.draftTransform = undefined;
  if (!e.selection) return;
  void e.edit({
    operation: "set_transform",
    clip_id: e.selection.id,
    transform,
  });
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
  ["x_milli", "Положение X", 0, 1000, (v) => `${v / 10} %`],
  ["y_milli", "Положение Y", 0, 1000, (v) => `${v / 10} %`],
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
