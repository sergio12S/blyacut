<script setup lang="ts">
import { computed, ref } from "vue";
import { useEditor } from "../stores/editor";
import { time } from "../types";
const e = useEditor();
const zoom = ref(65);
const ruler = ref<HTMLElement>();
const width = computed(() =>
  Math.max(900, (e.duration / 1000) * zoom.value + 160),
);
const ticks = computed(() =>
  Array.from(
    { length: Math.min(2000, Math.ceil(width.value / (zoom.value * 5))) },
    (_, i) => i * 5,
  ),
);
function seek(event: MouseEvent) {
  const r = ruler.value?.getBoundingClientRect();
  if (r) {
    e.playing = false;
    e.seek(((event.clientX - r.left) / zoom.value) * 1000);
  }
}
</script>
<template>
  <section class="timeline">
    <div class="timeline-toolbar">
      <div class="toolbar-group">
        <strong>Timeline</strong
        ><span class="badge">{{ e.clips.length }} clips</span
        ><button :disabled="!e.duration" @click="e.split()">
          ✂ Split <kbd>S</kbd></button
        ><button :disabled="!e.selection" @click="e.remove()">
          Delete <kbd>⌫</kbd></button
        ><button @click="e.addAudioTrack()">＋ Audio track</button>
      </div>
      <label class="zoom"
        >−
        <input
          v-model.number="zoom"
          type="range"
          min="15"
          max="160"
          aria-label="Timeline zoom"
        />
        +</label
      >
    </div>
    <div class="tracks-scroll">
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
              >{{ track.name
              }}<small>{{
                track.kind === "audio"
                  ? "Independent audio"
                  : "Video + source audio"
              }}</small></span
            >
          </div>
        </div>
        <div class="timeline-scroll">
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
              class="track-lane"
              :class="{ audio: track.kind === 'audio' }"
              @pointerdown.self="seek"
            >
              <button
                v-for="(c, index) in track.clips"
                :key="c.id"
                class="clip"
                :class="{
                  selected: e.selected === c.id,
                  audio: track.kind === 'audio',
                }"
                :style="{
                  left: (c.position_ms / 1000) * zoom + 'px',
                  width:
                    ((c.source_end_ms - c.source_start_ms) / 1000) * zoom +
                    'px',
                }"
                :draggable="track.kind === 'video'"
                @dragstart="(ev) => ev.dataTransfer?.setData('text/clip', c.id)"
                @dragover.prevent
                @drop.prevent="
                  (ev) => {
                    const id = ev.dataTransfer?.getData('text/clip');
                    if (id && track.kind === 'video')
                      e.edit({ operation: 'reorder_clip', clip_id: id, index });
                  }
                "
                @click.stop="e.select(c)"
                @contextmenu.prevent="e.select(c)"
              >
                <span class="clip-label">{{
                  e.project?.assets.find((a) => a.id === c.asset_id)?.name
                }}</span
                ><span class="clip-duration"
                  >{{ time(c.source_end_ms - c.source_start_ms) }} ·
                  {{ c.gain_milli / 10 }}%</span
                >
                <div class="clip-stripes" /></button
              ><span v-if="!track.clips.length" class="timeline-hint">{{
                track.kind === "audio"
                  ? "Add music or voiceover from the media library"
                  : "Add footage from the media library"
              }}</span>
            </div>
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
    <div class="timeline-footer">
      <span
        >Video ripple · audio keeps its position · inspector controls
        timing</span
      ><span>SPACE play / pause &nbsp; S split &nbsp; ⌘Z undo</span>
    </div>
  </section>
</template>
