<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEditor } from "../stores/editor";
import { time, type Clip } from "../types";
import {
  AudioEffectPreview,
  previewIsApproximate,
  videoEffectStyle,
} from "../preview-effects";
const e = useEditor();
const video = ref<HTMLVideoElement>();
const previewError = ref("");
const effectAudio = new AudioEffectPreview((error) => {
  previewError.value = String(error);
  e.playing = false;
});
const audio = new Map<string, HTMLMediaElement>();
function setAudio(id: string, node: unknown) {
  if (node instanceof HTMLMediaElement) audio.set(id, node);
  else audio.delete(id);
}
function assetSource(id: string) {
  const a = e.project?.assets.find((a) => a.id === id);
  return a ? convertFileSrc(a.path) : "";
}
const clip = computed(() =>
  e.videoClips.find(
    (c) =>
      e.playhead >= c.position_ms &&
      e.playhead < c.position_ms + c.source_end_ms - c.source_start_ms,
  ),
);
const source = computed(() =>
  clip.value ? assetSource(clip.value.asset_id) : "",
);
const videoPreviewStyle = computed(() =>
  videoEffectStyle(clip.value?.effects ?? []),
);
const hasNonCssEffects = computed(() =>
  previewIsApproximate(clip.value?.effects ?? []),
);
function volume(c: Clip) {
  const duration = c.source_end_ms - c.source_start_ms,
    local = e.playhead - c.position_ms;
  let gain = c.gain_milli / 1000;
  if (c.fade_in_ms) gain *= Math.min(1, local / c.fade_in_ms);
  if (c.fade_out_ms) gain *= Math.min(1, (duration - local) / c.fade_out_ms);
  return Math.max(0, Math.min(1, gain));
}
function syncMedia(v: HTMLMediaElement, c: Clip, force: boolean) {
  const local = e.playhead - c.position_ms,
    active = local >= 0 && local < c.source_end_ms - c.source_start_ms;
  if (!active) {
    v.pause();
    effectAudio.silence(v);
    return;
  }
  if (v.readyState < 1) return;
  const desired = (c.source_start_ms + local) / 1000;
  const jumped = force || Math.abs(v.currentTime - desired) > 0.18;
  if (!v.seeking && jumped) v.currentTime = desired;
  try {
    effectAudio.update(v, c, volume(c), e.playing, jumped);
  } catch (error) {
    previewError.value = String(error);
    e.playing = false;
    v.pause();
    return;
  }
  if (e.playing && v.paused)
    void v.play().catch((err) => {
      e.playing = false;
      previewError.value = String(err);
    });
  if (!e.playing) v.pause();
}
function sync(force = false) {
  effectAudio.prune();
  if (video.value && clip.value) syncMedia(video.value, clip.value, force);
  for (const c of e.audioClips) {
    const node = audio.get(c.id);
    if (node) syncMedia(node, c, force);
  }
}
watch(
  () => [e.playhead, e.playing, source.value],
  () => sync(),
  { flush: "post" },
);
watch(source, () => (previewError.value = ""));
watch(video, (_node, previous) => {
  if (previous) {
    previous.pause();
    effectAudio.silence(previous);
  }
});
let frame = 0,
  anchorTime = 0,
  anchorPosition = 0;
watch(
  () => e.playing,
  (playing) => {
    if (playing && e.playhead >= e.duration) e.seek(0);
    anchorPosition = e.playhead;
    anchorTime = performance.now();
  },
  { flush: "sync" },
);
function tick(now: number) {
  if (e.playing) {
    e.seek(anchorPosition + now - anchorTime);
    if (e.playhead >= e.duration) e.playing = false;
  }
  frame = requestAnimationFrame(tick);
}
onMounted(() => (frame = requestAnimationFrame(tick)));
onUnmounted(() => {
  cancelAnimationFrame(frame);
  audio.forEach((node) => node.pause());
  effectAudio.dispose();
});
</script>
<template>
  <section class="preview">
    <div class="panel-title">
      <span>PROGRAM MONITOR</span
      ><span
        >{{ e.project?.settings.width }} × {{ e.project?.settings.height }} ·
        {{ e.project?.settings.fps_num }} fps</span
      >
    </div>
    <audio
      v-for="c in e.audioClips"
      :key="c.id"
      :ref="(node) => setAudio(c.id, node)"
      :src="assetSource(c.asset_id)"
      crossorigin="anonymous"
      preload="metadata"
      @loadedmetadata="sync(true)"
      @error="
        previewError =
          'An audio asset cannot be decoded by this WebView. Try WAV or AAC.'
      "
    />
    <div class="screen">
      <video
        v-if="source"
        ref="video"
        :src="source"
        :style="videoPreviewStyle"
        crossorigin="anonymous"
        playsinline
        preload="auto"
        @loadedmetadata="sync(true)"
        @error="
          previewError =
            'This media cannot be decoded by the system WebView. Try an H.264 MP4.'
        "
      />
      <div
        v-if="source && hasNonCssEffects"
        class="approx-badge"
        title="У клипа есть эффекты, которые применяются при экспорте через FFmpeg"
      >
        превью ≈
      </div>
      <div v-else-if="!source" class="empty">
        <div class="empty-symbol">▷</div>
        <strong>{{
          e.audioClips.length ? "Audio composition" : "Your story starts here"
        }}</strong>
        <p>
          {{
            e.audioClips.length
              ? "Playback includes independent audio tracks."
              : "Import media and add it to the timeline."
          }}
        </p>
      </div>
      <div v-if="previewError" class="preview-error">{{ previewError }}</div>
    </div>
    <div class="transport">
      <span class="mono">{{ time(e.playhead) }}</span>
      <div>
        <button title="Previous frame (←)" @click="e.step(-1)">❮</button
        ><button
          class="play"
          :disabled="!e.duration"
          @click="e.playing = !e.playing"
        >
          {{ e.playing ? "Ⅱ" : "▶" }}</button
        ><button title="Next frame (→)" @click="e.step(1)">❯</button>
      </div>
      <span class="mono muted">{{ time(e.duration) }}</span>
    </div>
  </section>
</template>
<style scoped>
.approx-badge {
  position: absolute;
  top: 10px;
  right: 10px;
  background: rgba(245, 158, 11, 0.9);
  color: #12141a;
  font-size: 10px;
  font-weight: 700;
  padding: 3px 7px;
  border-radius: 4px;
  pointer-events: none;
  z-index: 10;
  letter-spacing: 0.3px;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.5);
  font-family: var(--font-ui, sans-serif);
}
</style>
