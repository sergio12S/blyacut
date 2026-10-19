<script setup lang="ts">
import { computed } from "vue";
import { useEditor } from "../stores/editor";
/** Speed, direction and freeze frame for the selected clip. */
const e = useEditor();
const speeds = [250, 500, 750, 1000, 1250, 1500, 2000, 3000, 4000];
const still = computed(
  () => !!e.project?.assets.find((a) => a.id === e.selection?.asset_id)?.still,
);
const onMain = computed(
  () =>
    e.project?.tracks.find((t) => t.kind === "video")?.id ===
    e.selectionTrack?.id,
);
function set(speed: number, reverse: boolean) {
  if (e.selection) void e.setSpeed(e.selection.id, speed, reverse);
}
</script>
<template>
  <section v-if="e.selection && !still" class="speed">
    <h4>Speed</h4>
    <div class="row">
      <select
        :value="e.selection.speed_milli ?? 1000"
        aria-label="Clip speed"
        @change="
          set(
            Number(($event.target as HTMLSelectElement).value),
            !!e.selection.reverse,
          )
        "
      >
        <option v-for="s in speeds" :key="s" :value="s">
          {{ s / 1000 }}×{{ s === 1000 ? " (normal)" : "" }}
        </option>
      </select>
      <label class="check"
        ><input
          type="checkbox"
          :checked="!!e.selection.reverse"
          @change="
            set(
              e.selection.speed_milli ?? 1000,
              ($event.target as HTMLInputElement).checked,
            )
          "
        />
        Reverse</label
      >
    </div>
    <button
      v-if="onMain"
      title="Hold the frame under the playhead for 2 seconds"
      @click="e.freezeFrame(2000)"
    >
      ❄ Freeze frame (2 s) at playhead
    </button>
  </section>
</template>
<style scoped>
.speed {
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
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 10px;
  align-items: center;
}
select {
  font: inherit;
  font-size: 12px;
  padding: 6px;
  color: var(--text-primary);
  background: var(--surface-input);
  border: 1px solid var(--border-control);
  border-radius: var(--radius-control);
}
.check {
  display: flex;
  gap: 6px;
  align-items: center;
  font-size: 12px;
  color: var(--text-secondary);
}
.check input {
  accent-color: var(--accent);
}
button {
  font-size: 12px;
}
</style>
