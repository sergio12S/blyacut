<script setup lang="ts">
import { ref, watch } from "vue";
import { useEditor } from "../stores/editor";
const e = useEditor();
const gain = ref(100),
  fadeIn = ref(0),
  fadeOut = ref(0),
  position = ref(0),
  trackId = ref("");
watch(
  () => e.selection,
  (c) => {
    gain.value = (c?.gain_milli ?? 1000) / 10;
    fadeIn.value = c?.fade_in_ms ?? 0;
    fadeOut.value = c?.fade_out_ms ?? 0;
    position.value = c?.position_ms ?? 0;
    trackId.value = e.selectionTrack?.id ?? "";
  },
  { immediate: true },
);
function applyAudio() {
  void e.edit({
    operation: "set_audio",
    clip_id: e.selected,
    gain_milli: Math.round(gain.value * 10),
    fade_in_ms: fadeIn.value,
    fade_out_ms: fadeOut.value,
  });
}
function move() {
  void e.edit({
    operation: "move_clip",
    clip_id: e.selected,
    track_id: trackId.value,
    position_ms: position.value,
  });
}
</script>
<template>
  <div class="audio-controls">
    <label
      >Volume <small>%</small
      ><input
        v-model.number="gain"
        aria-label="Clip volume percent"
        type="number"
        min="0"
        max="100"
        step="1"
    /></label>
    <div class="paired-fields">
      <label
        >Fade in <small>ms</small
        ><input
          v-model.number="fadeIn"
          aria-label="Fade in milliseconds"
          type="number"
          min="0" /></label
      ><label
        >Fade out <small>ms</small
        ><input
          v-model.number="fadeOut"
          aria-label="Fade out milliseconds"
          type="number"
          min="0"
      /></label>
    </div>
    <button class="full" @click="applyAudio">Apply audio settings</button
    ><label
      >Position <small>timeline ms</small
      ><input
        v-model.number="position"
        aria-label="Clip timeline position milliseconds"
        type="number"
        min="0" /></label
    ><label
      >Track<select v-model="trackId" aria-label="Destination track">
        <option
          v-for="track in e.project?.tracks"
          :key="track.id"
          :value="track.id"
        >
          {{ track.name }}
        </option>
      </select></label
    ><button class="full" @click="move">Move clip</button>
  </div>
</template>
