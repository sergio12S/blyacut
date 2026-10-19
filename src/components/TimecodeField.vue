<script setup lang="ts">
import { ref, watch } from "vue";
import { time } from "../types";
/** Edits a millisecond value as MM:SS.mmm; commits on Enter or blur, ↑/↓ nudge by 0.1 s (⇧ 1 s). */
const props = defineProps<{
  label: string;
  value: number;
  readonly?: boolean;
}>();
const emit = defineEmits<{ commit: [ms: number] }>();
const text = ref(time(props.value));
const invalid = ref(false);
watch(
  () => props.value,
  (v) => {
    text.value = time(v);
    invalid.value = false;
  },
);
function parse(input: string): number | null {
  const parts = input.trim().replace(",", ".").split(":");
  if (parts.length > 3 || parts.some((p) => !/^\d+(\.\d+)?$/.test(p)))
    return null;
  const seconds = parts.reduce((total, p) => total * 60 + Number(p), 0);
  return Number.isFinite(seconds) ? Math.round(seconds * 1000) : null;
}
function commit() {
  const ms = parse(text.value);
  invalid.value = ms === null;
  if (ms !== null && ms !== props.value) emit("commit", ms);
  else if (ms !== null) text.value = time(ms);
}
function nudge(event: KeyboardEvent) {
  const ms = parse(text.value);
  if (ms === null) return;
  const step =
    (event.shiftKey ? 1000 : 100) * (event.key === "ArrowUp" ? 1 : -1);
  text.value = time(Math.max(0, ms + step));
}
</script>
<template>
  <label class="timecode-field">
    <span>{{ label }}</span>
    <input
      v-model="text"
      :readonly="readonly"
      :aria-invalid="invalid"
      spellcheck="false"
      @keydown.enter.prevent="commit"
      @keydown.up.prevent="nudge"
      @keydown.down.prevent="nudge"
      @keydown.esc="text = time(value)"
      @blur="commit"
    />
  </label>
</template>
<style scoped>
.timecode-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 11px;
  color: var(--text-secondary);
}
input {
  font:
    12px ui-monospace,
    SFMono-Regular,
    Menlo,
    monospace;
  padding: 6px 8px;
  width: 100%;
}
input[readonly] {
  color: var(--text-secondary);
  border-style: dashed;
}
input[aria-invalid="true"] {
  border-color: var(--danger);
}
</style>
