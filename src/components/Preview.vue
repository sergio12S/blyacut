<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEditor } from "../stores/editor";
import { FULL_FRAME, isOverlayTrack, time, type Clip } from "../types";
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
const activeAt = (c: Clip) =>
  e.playhead >= c.position_ms &&
  e.playhead < c.position_ms + c.source_end_ms - c.source_start_ms;
const asset = (c: Clip) => e.project?.assets.find((a) => a.id === c.asset_id);
const mainTrack = computed(() =>
  e.project?.tracks.find((t) => t.kind === "video"),
);
const clip = computed(() => mainTrack.value?.clips.find(activeAt));
const mainStill = computed(() => !!(clip.value && asset(clip.value)?.still));
const source = computed(() =>
  clip.value && !mainStill.value ? assetSource(clip.value.asset_id) : "",
);
// Overlay layers in track order (later tracks draw on top), as export composites them.
const overlays = computed(() =>
  (e.project?.tracks ?? [])
    .filter((t) => isOverlayTrack(e.project, t))
    .flatMap((t) => t.clips.filter(activeAt)),
);
// The frame box has the project's aspect ratio and fits the screen.
const screen = ref<HTMLElement>();
const safeZones = ref(false);
const vertical = computed(
  () => (e.project?.settings.height ?? 0) > (e.project?.settings.width ?? 1),
);
const box = ref({ width: 0, height: 0 });
let observer: ResizeObserver | undefined;
function fit() {
  const s = e.project?.settings;
  const el = screen.value;
  if (!s || !el) return;
  const scale = Math.min(el.clientWidth / s.width, el.clientHeight / s.height);
  box.value = { width: s.width * scale, height: s.height * scale };
}
watch(() => e.project?.settings, fit);
watch(screen, (el) => {
  observer?.disconnect();
  if (el) {
    observer = new ResizeObserver(fit);
    observer.observe(el);
  }
});
/** Size/position of one overlay inside the frame box, mirroring the FFmpeg overlay maths. */
function layerStyle(c: Clip) {
  const s = e.project?.settings;
  const a = asset(c);
  if (!s || !a) return {};
  const draft = e.draftTransform;
  const t =
    (draft?.clipId === c.id ? draft.transform : c.transform) ?? FULL_FRAME;
  const fitScale = Math.min(s.width / a.width, s.height / a.height);
  const k = (box.value.width / s.width) * fitScale * (t.scale_milli / 1000);
  const width = a.width * k,
    height = a.height * k;
  const css = videoEffectStyle(c.effects);
  return {
    width: `${width}px`,
    height: `${height}px`,
    left: `${(box.value.width * t.x_milli) / 1000 - width / 2}px`,
    top: `${(box.value.height * t.y_milli) / 1000 - height / 2}px`,
    opacity: t.opacity_milli / 1000,
    filter: css.filter,
    transform: css.transform,
  };
}
const layerVideos = new Map<string, HTMLVideoElement>();
function setLayer(id: string, node: unknown) {
  if (node instanceof HTMLVideoElement) layerVideos.set(id, node);
  else layerVideos.delete(id);
}
function syncLayers() {
  for (const c of overlays.value) {
    const v = layerVideos.get(c.id);
    if (!v || v.readyState < 1) continue;
    const desired = (c.source_start_ms + e.playhead - c.position_ms) / 1000;
    if (!v.seeking && Math.abs(v.currentTime - desired) > 0.2)
      v.currentTime = desired;
    if (e.playing && v.paused) void v.play().catch(() => {});
    if (!e.playing) v.pause();
  }
}
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
  syncLayers();
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
onMounted(() => {
  frame = requestAnimationFrame(tick);
  fit();
});
onUnmounted(() => {
  observer?.disconnect();
  cancelAnimationFrame(frame);
  audio.forEach((node) => node.pause());
  effectAudio.dispose();
});
</script>
<template>
  <section class="preview">
    <div class="panel-title">
      <span
        >МОНИТОР<button
          class="safe-toggle"
          :aria-pressed="safeZones"
          title="Показать, что закроет интерфейс соцсетей (вертикаль) или рамку безопасных титров"
          @click="safeZones = !safeZones"
        >
          Безопасная зона
        </button></span
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
          'Этот звук WebView не может воспроизвести. Попробуйте WAV или AAC.'
      "
    />
    <div ref="screen" class="screen">
      <div
        v-if="clip || overlays.length"
        class="frame"
        :style="{ width: box.width + 'px', height: box.height + 'px' }"
      >
        <img
          v-if="mainStill && clip"
          class="layer-main"
          :class="{ fill: e.project?.settings.fill }"
          :src="assetSource(clip.asset_id)"
          :style="videoPreviewStyle"
          alt=""
        />
        <video
          v-if="source"
          ref="video"
          :class="{ fill: e.project?.settings.fill }"
          :src="source"
          :style="videoPreviewStyle"
          crossorigin="anonymous"
          playsinline
          preload="auto"
          @loadedmetadata="sync(true)"
          @error="
            previewError =
              'Это видео WebView не может воспроизвести. Попробуйте H.264 MP4 (экспорт всё равно сработает).'
          "
        />
        <div
          v-if="safeZones"
          class="safe"
          :class="{ vertical }"
          aria-hidden="true"
        >
          <template v-if="vertical">
            <i class="top" /><i class="bottom"
              ><span>интерфейс соцсетей</span></i
            ><i class="right" />
          </template>
          <i v-else class="title-safe" />
        </div>
        <template v-for="c in overlays" :key="c.id">
          <img
            v-if="asset(c)?.still"
            class="layer"
            :src="assetSource(c.asset_id)"
            :style="layerStyle(c)"
            alt=""
          />
          <video
            v-else
            :ref="(node) => setLayer(c.id, node)"
            class="layer"
            :src="assetSource(c.asset_id)"
            :style="layerStyle(c)"
            muted
            playsinline
            preload="auto"
            @loadedmetadata="syncLayers"
          />
        </template>
      </div>
      <div
        v-if="source && hasNonCssEffects"
        class="approx-badge"
        title="У клипа есть эффекты, которые применяются при экспорте через FFmpeg"
      >
        превью ≈
      </div>
      <div v-else-if="!source && !clip && !overlays.length" class="empty">
        <div class="empty-symbol">▷</div>
        <strong>{{
          e.audioClips.length ? "Только звук" : "Здесь будет ваше видео"
        }}</strong>
        <p>
          {{
            e.audioClips.length
              ? "Воспроизведение включает звуковые дорожки."
              : "Импортируйте медиа или создайте титр в библиотеке слева."
          }}
        </p>
      </div>
      <div v-if="previewError" class="preview-error">{{ previewError }}</div>
    </div>
    <div class="transport">
      <span class="mono">{{ time(e.playhead) }}</span>
      <div>
        <button title="Кадр назад (←)" @click="e.step(-1)">❮</button
        ><button
          class="play"
          :disabled="!e.duration"
          @click="e.playing = !e.playing"
        >
          {{ e.playing ? "Ⅱ" : "▶" }}</button
        ><button title="Кадр вперёд (→)" @click="e.step(1)">❯</button>
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
.frame {
  position: absolute;
  overflow: hidden;
  z-index: 1;
  background: #000;
}
.frame > video:not(.layer),
.layer-main {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: contain;
}
.frame > video.fill:not(.layer),
.frame > .layer-main.fill {
  object-fit: cover;
}
.layer {
  position: absolute;
  object-fit: fill;
  pointer-events: none;
}
.approx-badge {
  z-index: 3;
}
.safe-toggle {
  margin-left: 10px;
  padding: 1px 7px;
  font-size: 10px;
  letter-spacing: 0;
}
.safe-toggle[aria-pressed="true"] {
  color: var(--accent);
  border-color: #62744d;
}
.safe {
  position: absolute;
  inset: 0;
  z-index: 4;
  pointer-events: none;
}
.safe i {
  position: absolute;
  background: repeating-linear-gradient(
    45deg,
    #ff5a5a40 0 6px,
    #ff5a5a18 6px 12px
  );
}
.safe .top {
  left: 0;
  right: 0;
  top: 0;
  height: 8%;
}
.safe .bottom {
  left: 0;
  right: 0;
  bottom: 0;
  height: 22%;
}
.safe .bottom span {
  position: absolute;
  left: 6px;
  bottom: 4px;
  font-size: 10px;
  color: #ffd0d0;
}
.safe .right {
  right: 0;
  top: 30%;
  bottom: 22%;
  width: 14%;
}
.safe .title-safe {
  inset: 5%;
  background: none;
  border: 1px dashed #ffffff88;
}
</style>
