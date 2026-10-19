<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { save } from "@tauri-apps/plugin-dialog";
import { useEditor } from "./stores/editor";
import Preview from "./components/Preview.vue";
import Timeline from "./components/Timeline.vue";
import AgentPanel from "./components/AgentPanel.vue";
import ClipAudioControls from "./components/ClipAudioControls.vue";
import EffectsPanel from "./components/EffectsPanel.vue";
import LibraryPanel from "./components/LibraryPanel.vue";
import TimecodeField from "./components/TimecodeField.vue";
import type { Snapshot } from "./types";
import { time } from "./types";
const e = useEditor();
const exporting = ref(false),
  progress = ref(0),
  exportStatus = ref("");
const exportDialog = ref(false);
const cleanup: UnlistenFn[] = [];
let autosaveTimer: ReturnType<typeof setInterval> | undefined;
const autosavePath = ref("");
const dockTabs = [
  ["clip", "Клип"],
  ["audio", "Звук"],
  ["effects", "Эффекты"],
  ["agent", "Агент"],
] as const;
type Dock = (typeof dockTabs)[number][0];
const dock = ref<Dock>("clip");
const pendingEdits = ref(0);
function dockKey(event: KeyboardEvent) {
  if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
  event.preventDefault();
  event.stopPropagation();
  const ids = dockTabs.map(([id]) => id);
  const at = ids.indexOf(dock.value);
  const next =
    event.key === "Home"
      ? 0
      : event.key === "End"
        ? ids.length - 1
        : (at + (event.key === "ArrowRight" ? 1 : ids.length - 1)) % ids.length;
  dock.value = ids[next];
  document.getElementById(`${dock.value}-tab`)?.focus();
}
const assetName = computed(
  () =>
    e.project?.assets.find((a) => a.id === e.selection?.asset_id)?.name ?? "",
);
function trim(startMs: number, endMs: number) {
  if (!e.selection) return;
  if (endMs <= startMs) {
    e.error = "Конец клипа должен быть позже начала";
    return;
  }
  void e.edit({
    operation: "trim_clip",
    clip_id: e.selected,
    start_ms: startMs,
    end_ms: endMs,
  });
}
function key(ev: KeyboardEvent) {
  if ((ev.target as HTMLElement).matches("input,textarea,select")) return;
  const mod = ev.metaKey || ev.ctrlKey;
  if (mod && ev.key === "s") {
    ev.preventDefault();
    void e.saveProject(ev.shiftKey);
  } else if (mod && ev.key === "z") {
    ev.preventDefault();
    void e.history(ev.shiftKey);
  } else if (mod && ev.key === "o") {
    ev.preventDefault();
    void e.openProject();
  } else if (ev.code === "Space") {
    ev.preventDefault();
    if (e.duration) e.playing = !e.playing;
  } else if (ev.key === "s") void e.split();
  else if (ev.key === "Backspace" || ev.key === "Delete") {
    ev.preventDefault();
    void e.remove();
  } else if (ev.key === "ArrowLeft" || ev.key === "ArrowRight") {
    ev.preventDefault();
    e.step(ev.key === "ArrowLeft" ? -1 : 1);
  }
}
onMounted(async () => {
  window.addEventListener("keydown", key);
  if (!isTauri()) {
    e.error =
      "Запустите npm run tauri dev: в браузере нет доступа к файлам и движку монтажа.";
    return;
  }
  cleanup.push(
    await listen<Snapshot>("project-changed", ({ payload }) =>
      e.accept(payload),
    ),
  );
  await e.guard(e.refresh);
  autosaveTimer = setInterval(() => {
    if (e.dirty && !e.busy)
      void invoke<string>("autosave")
        .then((path) => (autosavePath.value = path))
        .catch((err) => (e.error = `Автосохранение не удалось: ${err}`));
  }, 30000);
  cleanup.push(
    await listen<{ progress: number; status: string; error: string | null }>(
      "render",
      ({ payload: p }) => {
        progress.value = p.progress;
        exporting.value = p.status === "rendering";
        exportStatus.value =
          p.error ?? (p.status === "complete" ? "Экспорт готов" : "Экспорт…");
        if (p.error) e.error = p.error;
      },
    ),
  );
  cleanup.push(
    await getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === "drop") void e.importPaths(payload.paths);
    }),
  );
});
onUnmounted(() => {
  window.removeEventListener("keydown", key);
  clearInterval(autosaveTimer);
  cleanup.forEach((f) => f());
});
async function render() {
  await e.guard(async () => {
    const path = await save({
      defaultPath: "BlyaCut-export.mp4",
      filters: [{ name: "Видео MP4", extensions: ["mp4"] }],
    });
    if (!path) return;
    progress.value = 0;
    exporting.value = true;
    exportStatus.value = "Экспорт…";
    try {
      await invoke("export_video", { path });
      exportDialog.value = false;
    } catch (err) {
      exporting.value = false;
      throw err;
    }
  });
}
</script>
<template>
  <div class="editor-shell">
    <header class="app-header">
      <div class="brand"><span class="brand-icon">B/</span>BlyaCut</div>
      <div class="project-name" :title="e.projectPath || 'Проект не сохранён'">
        {{ e.project?.name ?? "Новый проект"
        }}<span
          v-if="e.dirty"
          class="dirty"
          title="Есть несохранённые изменения"
        >
          ●</span
        >
      </div>
      <div class="history" role="group" aria-label="История">
        <button
          title="Отменить (⌘Z)"
          aria-label="Отменить"
          @click="e.history()"
        >
          ↶</button
        ><button
          title="Повторить (⇧⌘Z)"
          aria-label="Повторить"
          @click="e.history(true)"
        >
          ↷
        </button>
      </div>
      <div v-if="exporting || exportStatus" class="export-chip" role="status">
        <span>{{ exportStatus }}</span>
        <template v-if="exporting"
          ><span class="mono">{{ Math.round(progress * 100) }}%</span>
          <span class="chip-bar"
            ><i :style="{ width: progress * 100 + '%' }"
          /></span>
          <button @click="invoke('cancel_export')">Отмена</button></template
        ><button
          v-else
          class="ghost"
          aria-label="Скрыть"
          @click="exportStatus = ''"
        >
          ×
        </button>
      </div>
      <nav>
        <button :disabled="e.busy" title="⌘O" @click="e.openProject()">
          Открыть</button
        ><button
          :disabled="!e.project || e.busy"
          title="⌘S"
          @click="e.saveProject()"
        >
          Сохранить</button
        ><button
          class="primary"
          :disabled="!e.duration || exporting"
          @click="exportDialog = true"
        >
          ↑ Экспорт
        </button>
      </nav>
    </header>
    <div v-if="e.error" class="error-banner" role="alert">
      {{ e.error }}<button aria-label="Закрыть" @click="e.error = ''">×</button>
    </div>
    <div class="workspace">
      <LibraryPanel />
      <Preview />
      <aside class="inspector">
        <div
          class="dock-tabs"
          role="tablist"
          aria-label="Свойства"
          @keydown="dockKey"
        >
          <button
            v-for="[id, label] in dockTabs"
            :id="`${id}-tab`"
            :key="id"
            role="tab"
            :tabindex="dock === id ? 0 : -1"
            :aria-selected="dock === id"
            :aria-controls="`${id}-panel`"
            @click="dock = id"
          >
            {{ label
            }}<span
              v-if="id === 'effects' && e.selection?.effects.length"
              class="tab-count"
              >{{ e.selection.effects.length }}</span
            ><span
              v-if="id === 'agent' && pendingEdits"
              class="pending-count"
              >{{ pendingEdits }}</span
            >
          </button>
        </div>
        <div
          v-if="dock !== 'agent' && !e.selection"
          class="dock-content inspector-empty"
        >
          <p>Выберите клип на таймлайне.</p>
          <small>Эффекты можно перетащить из библиотеки прямо на клип.</small>
        </div>
        <div
          v-else-if="dock === 'clip' && e.selection"
          id="clip-panel"
          class="dock-content inspector-body"
          role="tabpanel"
          aria-labelledby="clip-tab"
        >
          <h3 :title="assetName">{{ assetName }}</h3>
          <p class="clip-kind">
            {{
              e.selectionTrack?.kind === "audio"
                ? "Звуковая дорожка · без сдвига соседей"
                : "Видеодорожка · соседи сдвигаются"
            }}
          </p>
          <div class="paired-fields">
            <TimecodeField
              label="Начало в исходнике"
              :value="e.selection.source_start_ms"
              @commit="trim($event, e.selection.source_end_ms)"
            />
            <TimecodeField
              label="Конец в исходнике"
              :value="e.selection.source_end_ms"
              @commit="trim(e.selection.source_start_ms, $event)"
            />
            <TimecodeField
              label="На таймлайне"
              :value="e.selection.position_ms"
              readonly
            />
            <TimecodeField
              label="Длительность"
              :value="e.selection.source_end_ms - e.selection.source_start_ms"
              readonly
            />
          </div>
          <p class="muted">Enter — применить · ↑/↓ — ±0,1 с (⇧ ±1 с)</p>
          <div class="clip-actions">
            <button title="S" @click="e.split()">✂ Разрезать</button
            ><button class="danger" title="⌫" @click="e.remove()">
              Удалить
            </button>
          </div>
        </div>
        <div
          v-else-if="dock === 'audio' && e.selection"
          id="audio-panel"
          class="dock-content inspector-body"
          role="tabpanel"
          aria-labelledby="audio-tab"
        >
          <ClipAudioControls />
        </div>
        <div
          v-else-if="dock === 'effects'"
          id="effects-panel"
          class="dock-content"
          role="tabpanel"
          aria-labelledby="effects-tab"
        >
          <EffectsPanel section="chain" />
        </div>
        <div
          v-show="dock === 'agent'"
          id="agent-panel"
          class="dock-content"
          role="tabpanel"
          aria-labelledby="agent-tab"
        >
          <AgentPanel @pending="pendingEdits = $event" />
        </div>
      </aside>
    </div>
    <Timeline />
    <footer class="status-bar">
      <span><i /> Локально</span
      ><span :title="autosavePath">{{
        autosavePath
          ? "Резервная копия сохранена"
          : "Rust · без изменения исходников"
      }}</span
      ><span>{{ e.projectPath || "Проект не сохранён" }}</span>
    </footer>
    <div
      v-if="exportDialog"
      class="modal-backdrop"
      @click.self="exportDialog = false"
    >
      <section class="export-dialog" role="dialog" aria-label="Экспорт">
        <span class="eyebrow">ЭКСПОРТ</span>
        <h2>Сохранить видео</h2>
        <p>H.264 · AAC · MP4</p>
        <div class="export-spec">
          <span
            >Размер<strong
              >{{ e.project?.settings.width }} ×
              {{ e.project?.settings.height }}</strong
            ></span
          ><span
            >Длина<strong>{{ time(e.duration) }}</strong></span
          ><span>Кодирование<strong>CPU · CRF 20</strong></span>
        </div>
        <p class="muted">
          Экспорт идёт со снимка таймлайна — монтаж можно продолжать. Нужен
          новый файл: существующие не перезаписываются.
        </p>
        <div class="dialog-actions">
          <button @click="exportDialog = false">Отмена</button
          ><button class="primary" @click="render()">
            Выбрать файл и экспортировать
          </button>
        </div>
      </section>
    </div>
  </div>
</template>
