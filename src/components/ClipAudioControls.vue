<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useEditor } from "../stores/editor";
import TimecodeField from "./TimecodeField.vue";
import { trackName } from "../types";
const e = useEditor();
const gain = ref(100),
  trackId = ref("");
watch(
  () => e.selection,
  (c) => {
    gain.value = (c?.gain_milli ?? 1000) / 10;
    trackId.value = e.selectionTrack?.id ?? "";
  },
  { immediate: true },
);
const tracks = computed(
  () =>
    e.project?.tracks.filter((t) => t.kind === e.selectionTrack?.kind) ?? [],
);
/** Each control commits immediately as one Undo step. */
function setAudio(change: {
  gain?: number;
  fadeIn?: number;
  fadeOut?: number;
}) {
  const c = e.selection;
  if (!c) return;
  void e.edit({
    operation: "set_audio",
    clip_id: c.id,
    gain_milli: Math.round((change.gain ?? gain.value) * 10),
    fade_in_ms: change.fadeIn ?? c.fade_in_ms,
    fade_out_ms: change.fadeOut ?? c.fade_out_ms,
  });
}
function move(positionMs = e.selection?.position_ms ?? 0) {
  if (!e.selection) return;
  void e.edit({
    operation: "move_clip",
    clip_id: e.selection.id,
    track_id: trackId.value,
    position_ms: positionMs,
  });
}
</script>
<template>
  <div v-if="e.selection" class="audio-controls">
    <label class="volume"
      ><span>Громкость</span>
      <input
        v-model.number="gain"
        type="range"
        min="0"
        max="100"
        step="1"
        aria-label="Громкость клипа, процентов"
        @change="setAudio({})"
      /><output>{{ gain }} %</output></label
    >
    <div class="pair">
      <TimecodeField
        label="Появление"
        :value="e.selection.fade_in_ms"
        @commit="setAudio({ fadeIn: $event })"
      />
      <TimecodeField
        label="Затухание"
        :value="e.selection.fade_out_ms"
        @commit="setAudio({ fadeOut: $event })"
      />
    </div>
    <div class="pair">
      <TimecodeField
        label="Позиция на таймлайне"
        :value="e.selection.position_ms"
        :readonly="e.selectionTrack?.kind === 'video'"
        @commit="move($event)"
      />
      <label class="track"
        ><span>Дорожка</span
        ><select
          v-model="trackId"
          aria-label="Дорожка клипа"
          :disabled="tracks.length < 2"
          @change="move()"
        >
          <option v-for="t in tracks" :key="t.id" :value="t.id">
            {{ trackName(t.name) }}
          </option>
        </select></label
      >
    </div>
    <label v-if="e.selectionTrack?.kind === 'audio'" class="track"
      ><span>Приглушать под голос</span
      ><select
        :value="e.selection.duck_db ?? 0"
        aria-label="Приглушение под голос"
        @change="
          e.edit({
            operation: 'set_ducking',
            clip_id: e.selection!.id,
            duck_db: Number(($event.target as HTMLSelectElement).value),
          })
        "
      >
        <option :value="0">Выкл.</option>
        <option v-for="db in [6, 12, 18, 24]" :key="db" :value="db">
          на {{ db }} дБ, пока звучит голос
        </option>
      </select></label
    >
    <p class="note">
      Изменения применяются сразу. Видео двигается перетаскиванием на таймлайне.
      Голос — это звук главного видео и неприглушаемые звуковые клипы; в
      мониторе приглушение не слышно, только в экспорте.
    </p>
  </div>
</template>
<style scoped>
.audio-controls {
  display: flex;
  flex-direction: column;
  gap: 12px;
  border: 0;
  padding: 0;
}
.volume {
  display: grid;
  grid-template-columns: 1fr;
  gap: 4px;
  font-size: 11px;
  color: var(--text-secondary);
}
.volume input {
  accent-color: var(--accent);
  width: 100%;
}
.volume output {
  font: 12px var(--font-code);
  color: var(--text-primary);
  justify-self: end;
  margin-top: -2px;
}
.pair {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}
.track {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 11px;
  color: var(--text-secondary);
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
.note {
  margin: 0;
  font-size: 11px;
  color: var(--text-muted);
}
</style>
