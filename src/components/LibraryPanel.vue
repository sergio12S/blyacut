<script setup lang="ts">
import { ref } from "vue";
import { useEditor } from "../stores/editor";
import EffectsPanel from "./EffectsPanel.vue";
import { time } from "../types";
const e = useEditor();
const tab = ref<"media" | "titles" | "effects">("media");
const tabs = [
  ["media", "Медиа"],
  ["titles", "Титры"],
  ["effects", "Эффекты"],
] as const;
// Title styles: text colour on background colour.
const styles = [
  ["Классика", "#FFFFFF", "#000000"],
  ["Неон", "#00FFE0", "#0B0C16"],
  ["Лайм", "#18230C", "#C1F17B"],
  ["Закат", "#FFE3B3", "#5A1E3A"],
  ["Газета", "#111111", "#F2EDE4"],
  ["Синий", "#FFFFFF", "#1F4FD8"],
] as const;
const titleText = ref("BlyaCut");
const style = ref(0);
const seconds = ref(3);
const overlay = ref(false);
const position = ref<"top" | "center" | "bottom">("bottom");
const backing = ref<"box" | "outline" | "shadow">("box");
function createTitle() {
  const [, text, background] = styles[style.value];
  if (overlay.value)
    void e.createTextOverlay(
      titleText.value,
      text === "#111111" || text === "#18230C" ? background : text,
      position.value,
      seconds.value * 1000,
      backing.value,
    );
  else
    void e.createTitle(titleText.value, text, background, seconds.value * 1000);
}
</script>
<template>
  <aside class="library" aria-label="Библиотека">
    <div class="lib-tabs" role="tablist">
      <button
        v-for="[id, label] in tabs"
        :key="id"
        role="tab"
        :aria-selected="tab === id"
        @click="tab = id"
      >
        {{ label }}
      </button>
    </div>

    <div v-if="tab === 'media'" class="lib-body">
      <button class="import" :disabled="e.busy" @click="e.importMedia()">
        ＋ Импорт медиа
      </button>
      <p class="hint">или перетащите видео и аудио в окно</p>
      <div class="media-list">
        <div
          v-for="asset in e.project?.assets"
          :key="asset.id"
          class="media-row"
        >
          <div class="thumb" :class="{ audio: !asset.width }">
            <span>{{ time(asset.duration_ms).slice(0, 5) }}</span>
          </div>
          <div class="meta">
            <strong :title="asset.path">{{ asset.name }}</strong>
            <small>{{
              asset.width ? `${asset.width}×${asset.height}` : "аудио"
            }}</small>
          </div>
          <button
            class="add"
            :disabled="e.busy"
            title="Добавить в конец таймлайна"
            :aria-label="`Добавить ${asset.name} на таймлайн`"
            @click="e.append(asset.id)"
          >
            ＋
          </button>
          <button
            v-if="asset.width"
            class="add"
            :disabled="e.busy"
            title="Поверх видео в позиции плейхеда (картинка в картинке, логотип)"
            :aria-label="`Наложить ${asset.name} поверх видео`"
            @click="e.placeOverlay(asset.id)"
          >
            ⧉
          </button>
          <span v-else />
        </div>
        <p v-if="!e.project?.assets.length" class="empty">
          Здесь появятся ваши файлы.<br />Видео · WAV · MP3 · M4A · FLAC
        </p>
      </div>
      <p class="footer">● Исходные файлы не изменяются</p>
    </div>

    <div v-else-if="tab === 'titles'" class="lib-body">
      <label class="field"
        >Текст (до 4 строк)
        <textarea v-model="titleText" rows="3" maxlength="200" />
      </label>
      <div class="styles" role="radiogroup" aria-label="Стиль титра">
        <button
          v-for="([name, text, background], i) in styles"
          :key="name"
          role="radio"
          :aria-checked="style === i"
          :title="name"
          :style="{ color: text, background }"
          @click="style = i"
        >
          Аа
        </button>
      </div>
      <div class="mode" role="radiogroup" aria-label="Вид титра">
        <button role="radio" :aria-checked="!overlay" @click="overlay = false">
          Отдельный кадр
        </button>
        <button role="radio" :aria-checked="overlay" @click="overlay = true">
          Поверх видео
        </button>
      </div>
      <label v-if="overlay" class="field"
        >Где
        <select v-model="position">
          <option value="top">Сверху</option>
          <option value="center">По центру</option>
          <option value="bottom">Снизу (субтитр)</option>
        </select>
      </label>
      <label v-if="overlay" class="field"
        >Читаемость
        <select v-model="backing">
          <option value="box">Плашка — читается на любом фоне</option>
          <option value="outline">Обводка</option>
          <option value="shadow">Только тень</option>
        </select>
      </label>
      <label class="field"
        >Длительность
        <select v-model.number="seconds">
          <option v-for="s in [2, 3, 4, 5, 8]" :key="s" :value="s">
            {{ s }} с
          </option>
        </select>
      </label>
      <button
        class="primary create"
        :disabled="e.busy || !titleText.trim()"
        @click="createTitle"
      >
        {{ overlay ? "Наложить текст" : "Создать титр" }}
      </button>
      <p class="hint">
        {{
          overlay
            ? "Текст встанет поверх видео в позиции плейхеда."
            : "Титр встанет в конец видеодорожки. Добавьте ему «Появление из чёрного» во вкладке «Эффекты»."
        }}
      </p>
    </div>

    <div v-else class="lib-body effects">
      <EffectsPanel section="library" />
    </div>
  </aside>
</template>
<style scoped>
.library {
  display: flex;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
  background: var(--surface-panel);
  border-right: 1px solid var(--border-subtle);
}
.lib-tabs {
  display: flex;
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}
.lib-tabs button {
  flex: 1;
  border: 0;
  border-radius: 0;
  background: transparent;
  color: var(--text-secondary);
  padding: 10px 4px;
  font-size: 12px;
}
.lib-tabs button[aria-selected="true"] {
  color: var(--text-primary);
  box-shadow: inset 0 -2px var(--accent);
}
.lib-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.lib-body.effects {
  padding: 0;
}
.import {
  width: 100%;
  border-color: #4d6a2a;
  color: var(--accent);
  background: #1f2619;
}
.hint {
  margin: -4px 0 0;
  font-size: 11px;
  color: var(--text-muted);
  text-align: center;
}
.media-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.media-row {
  display: grid;
  grid-template-columns: 52px minmax(0, 1fr) 28px 28px;
  gap: 8px;
  align-items: center;
  padding: 5px;
  border-radius: var(--radius-control);
}
.media-row:hover {
  background: var(--surface-raised);
}
.thumb {
  height: 32px;
  border-radius: 4px;
  background: linear-gradient(
    135deg,
    var(--video-border),
    var(--video-surface)
  );
  position: relative;
}
.thumb.audio {
  background: repeating-linear-gradient(
    90deg,
    var(--audio-border) 0 2px,
    var(--audio-surface) 2px 4px
  );
}
.thumb span {
  position: absolute;
  right: 2px;
  bottom: 1px;
  font: 10px var(--font-code);
  background: #000a;
  padding: 0 3px;
  border-radius: 2px;
}
.meta {
  min-width: 0;
  display: flex;
  flex-direction: column;
}
.meta strong {
  font-size: 12px;
  font-weight: 550;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.meta small {
  font-size: 11px;
  color: var(--text-muted);
}
.add {
  width: 28px;
  height: 28px;
  padding: 0;
}
.empty {
  margin: 24px 0;
  text-align: center;
  color: var(--text-muted);
  font-size: 12px;
  line-height: 1.6;
}
.footer {
  margin: auto 0 0;
  font-size: 11px;
  color: var(--text-muted);
}
.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 11px;
  color: var(--text-secondary);
}
textarea,
select {
  font: inherit;
  font-size: 13px;
  color: var(--text-primary);
  background: var(--surface-input);
  border: 1px solid var(--border-control);
  border-radius: var(--radius-control);
  padding: 7px 8px;
  resize: vertical;
}
.styles {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 6px;
}
.styles button {
  height: 40px;
  font-weight: 700;
  font-size: 15px;
  border: 1px solid var(--border-control);
}
.styles button[aria-checked="true"] {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}
.create {
  width: 100%;
}
.mode {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 4px;
}
.mode button {
  padding: 6px 4px;
  font-size: 12px;
}
.mode button[aria-checked="true"] {
  border-color: var(--accent);
  color: var(--accent);
}
/* The gallery fills the library column instead of a fixed-height box. */
.effects :deep(.effects-panel) {
  gap: 0;
}
.effects :deep(.section-block) {
  border-width: 0 0 1px;
  border-radius: 0;
  background: transparent;
}
.effects :deep(.presets-grid) {
  max-height: none;
}
</style>
