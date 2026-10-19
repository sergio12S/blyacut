<script setup lang="ts">
import { computed } from "vue";
import { useEditor } from "../stores/editor";
import type { TransitionKind } from "../types";
import { TRANSITION_KINDS, TRANSITION_NAMES } from "../transitions";
/** Transition into the selected main-track clip; hidden for the first clip. */
const e = useEditor();
const index = computed(() => {
  const main = e.project?.tracks.find((t) => t.kind === "video");
  return main?.clips.findIndex((c) => c.id === e.selection?.id) ?? -1;
});
const current = computed(() => e.selection?.transition_in ?? null);
function set(kind: string, ms = current.value?.duration_ms ?? 800) {
  if (e.selection)
    void e.setTransition(
      e.selection.id,
      kind ? (kind as TransitionKind) : null,
      ms,
    );
}
</script>
<template>
  <section v-if="index > 0" class="transition-control">
    <h4>Переход в начале</h4>
    <div class="row">
      <select
        :value="current?.kind ?? ''"
        aria-label="Вид перехода"
        @change="set(($event.target as HTMLSelectElement).value)"
      >
        <option value="">Без перехода (склейка)</option>
        <option v-for="k in TRANSITION_KINDS" :key="k" :value="k">
          {{ TRANSITION_NAMES[k] }}
        </option>
      </select>
      <select
        :value="current?.duration_ms ?? 800"
        :disabled="!current"
        aria-label="Длительность перехода"
        @change="
          set(current!.kind, Number(($event.target as HTMLSelectElement).value))
        "
      >
        <option
          v-for="ms in [300, 500, 800, 1000, 1500, 2000]"
          :key="ms"
          :value="ms"
        >
          {{ ms / 1000 }} с
        </option>
      </select>
    </div>
  </section>
</template>
<style scoped>
.transition-control {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 10px;
  border-top: 1px solid var(--border-subtle);
}
h4 {
  margin: 0;
  font-size: 12px;
  color: var(--text-secondary);
  font-weight: 600;
}
.row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 72px;
  gap: 6px;
}
select {
  font: inherit;
  font-size: 12px;
  padding: 6px;
  color: var(--text-primary);
  background: var(--surface-input);
  border: 1px solid var(--border-control);
  border-radius: var(--radius-control);
  min-width: 0;
}
</style>
