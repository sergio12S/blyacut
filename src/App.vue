<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
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
import TransformControls from "./components/TransformControls.vue";
import TransitionControl from "./components/TransitionControl.vue";
import SpeedControl from "./components/SpeedControl.vue";
import type { Snapshot } from "./types";
import { isOverlayTrack, time } from "./types";
import logo from "./assets/logo-mark.svg";
import { clipLength } from "./timeline-gesture";
const e = useEditor();
const exporting = ref(false),
  progress = ref(0),
  exportStatus = ref("");
const exportDialog = ref(false);
const cleanup: UnlistenFn[] = [];
let autosaveTimer: ReturnType<typeof setInterval> | undefined;
const autosavePath = ref("");
const dockTabs = [
  ["clip", "Clip"],
  ["audio", "Audio"],
  ["effects", "Effects"],
  ["agent", "Agent"],
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
const overlaySelected = computed(
  () => !!e.selectionTrack && isOverlayTrack(e.project, e.selectionTrack),
);
// Output presets; vertical and square crop landscape footage to fill the frame.
const formats = [
  {
    key: "1920x1080@30",
    label: "Landscape 1080p · 30 fps",
    w: 1920,
    h: 1080,
    fps: 30,
    fill: false,
  },
  {
    key: "1920x1080@60",
    label: "Landscape 1080p · 60 fps",
    w: 1920,
    h: 1080,
    fps: 60,
    fill: false,
  },
  {
    key: "1280x720@30",
    label: "Landscape 720p · 30 fps",
    w: 1280,
    h: 720,
    fps: 30,
    fill: false,
  },
  {
    key: "3840x2160@30",
    label: "4K UHD · 30 fps",
    w: 3840,
    h: 2160,
    fps: 30,
    fill: false,
  },
  {
    key: "1080x1920@30+fill",
    label: "Vertical 9:16 (Shorts, TikTok, Reels)",
    w: 1080,
    h: 1920,
    fps: 30,
    fill: true,
  },
  {
    key: "1080x1080@30+fill",
    label: "Square 1:1",
    w: 1080,
    h: 1080,
    fps: 30,
    fill: true,
  },
];
const formatKey = computed(() => {
  const s = e.project?.settings;
  if (!s) return "";
  const fps = Math.round(s.fps_num / s.fps_den);
  return `${s.width}x${s.height}@${fps}${s.fill ? "+fill" : ""}`;
});
function applyFormat(key: string) {
  const f = formats.find((f) => f.key === key);
  if (f)
    void e.setSettings({
      width: f.w,
      height: f.h,
      fps_num: f.fps,
      fps_den: 1,
      fill: f.fill,
    });
}
const assetName = computed(
  () =>
    e.project?.assets.find((a) => a.id === e.selection?.asset_id)?.name ?? "",
);
function trim(startMs: number, endMs: number) {
  if (!e.selection) return;
  if (endMs <= startMs) {
    e.error = "The clip end must come after its start";
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
  if ((ev.target as Element | null)?.closest?.("input,textarea,select")) return;
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
// The name is the joke: «Бля!» ("Blya!") greets you on launch (switch it off in the status bar).
const greeting = ref(readGreeting());
function readGreeting() {
  try {
    return localStorage.getItem("blyacut.greeting") !== "off";
  } catch {
    return true;
  }
}
watch(greeting, (on) => {
  try {
    localStorage.setItem("blyacut.greeting", on ? "on" : "off");
  } catch {
    /* storage unavailable: keep the in-memory choice */
  }
});
async function sayBlya() {
  const text = "Бля!";
  if (
    isTauri() &&
    (await invoke<boolean>("greet", { text }).catch(() => false))
  )
    return;
  if ("speechSynthesis" in window) {
    const u = new SpeechSynthesisUtterance(text);
    u.lang = "ru-RU";
    speechSynthesis.speak(u);
  }
}
onMounted(async () => {
  window.addEventListener("keydown", key);
  if (greeting.value) void sayBlya();
  if (!isTauri()) {
    e.error =
      "Run npm run tauri dev: the browser has no file access or editing engine.";
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
        .catch((err) => (e.error = `Autosave failed: ${err}`));
  }, 30000);
  cleanup.push(
    await listen<{ progress: number; status: string; error: string | null }>(
      "render",
      ({ payload: p }) => {
        progress.value = p.progress;
        exporting.value = p.status === "rendering";
        exportStatus.value =
          p.error ??
          (p.status === "complete" ? "Export complete" : "Exporting…");
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
      filters: [{ name: "MP4 video", extensions: ["mp4"] }],
    });
    if (!path) return;
    progress.value = 0;
    exporting.value = true;
    exportStatus.value = "Exporting…";
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
      <div class="brand" title="BlyaCut: editing so good you say “blya!”">
        <img
          class="brand-mark"
          :src="logo"
          alt=""
          width="26"
          height="26"
        /><span class="wordmark">Blya<b>Cut</b></span>
      </div>
      <div class="project-name" :title="e.projectPath || 'Project not saved'">
        {{ e.project?.name ?? "Untitled project"
        }}<span v-if="e.dirty" class="dirty" title="Unsaved changes"> ●</span>
      </div>
      <div class="history" role="group" aria-label="History">
        <button title="Undo (⌘Z)" aria-label="Undo" @click="e.history()">
          ↶</button
        ><button title="Redo (⇧⌘Z)" aria-label="Redo" @click="e.history(true)">
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
          <button @click="invoke('cancel_export')">Cancel</button></template
        ><button
          v-else
          class="ghost"
          aria-label="Dismiss"
          @click="exportStatus = ''"
        >
          ×
        </button>
      </div>
      <nav>
        <button :disabled="e.busy" title="⌘O" @click="e.openProject()">
          Open</button
        ><button
          :disabled="!e.project || e.busy"
          title="⌘S"
          @click="e.saveProject()"
        >
          Save</button
        ><button
          class="primary"
          :disabled="!e.duration || exporting"
          @click="exportDialog = true"
        >
          ↑ Export
        </button>
      </nav>
    </header>
    <div v-if="e.error" class="error-banner" role="alert">
      {{ e.error }}<button aria-label="Close" @click="e.error = ''">×</button>
    </div>
    <div class="workspace">
      <LibraryPanel />
      <Preview />
      <aside class="inspector">
        <div
          class="dock-tabs"
          role="tablist"
          aria-label="Properties"
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
          <p>Select a clip on the timeline.</p>
          <small
            >You can drag effects from the library straight onto a clip.</small
          >
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
                ? "Audio track · neighbours stay put"
                : overlaySelected
                  ? "Overlay · above the main video, silent"
                  : "Main video track · neighbours ripple"
            }}
          </p>
          <div class="paired-fields">
            <TimecodeField
              label="Source in"
              :value="e.selection.source_start_ms"
              @commit="trim($event, e.selection.source_end_ms)"
            />
            <TimecodeField
              label="Source out"
              :value="e.selection.source_end_ms"
              @commit="trim(e.selection.source_start_ms, $event)"
            />
            <TimecodeField
              label="On timeline"
              :value="e.selection.position_ms"
              readonly
            />
            <TimecodeField
              label="Duration"
              :value="clipLength(e.selection)"
              readonly
            />
          </div>
          <p class="muted">Enter applies · ↑/↓ ±0.1 s (⇧ ±1 s)</p>
          <TransformControls v-if="overlaySelected" />
          <TransitionControl v-else />
          <SpeedControl />
          <div class="clip-actions">
            <button title="S" @click="e.split()">✂ Split</button
            ><button class="danger" title="⌫" @click="e.remove()">
              Delete
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
      <label class="greet-toggle" title="Say “Blya!” on launch"
        ><input v-model="greeting" type="checkbox" /> “Blya!” on launch</label
      ><span><i /> Local</span
      ><span :title="autosavePath">{{
        autosavePath
          ? "Recovery snapshot saved"
          : "Rust core · originals untouched"
      }}</span
      ><span>{{ e.projectPath || "Project not saved" }}</span>
    </footer>
    <div
      v-if="exportDialog"
      class="modal-backdrop"
      @click.self="exportDialog = false"
    >
      <section class="export-dialog" role="dialog" aria-label="Export">
        <span class="eyebrow">EXPORT</span>
        <h2>Save your video</h2>
        <p>H.264 · AAC · MP4</p>
        <label class="format"
          >Format
          <select
            :value="formatKey"
            aria-label="Project format"
            @change="applyFormat(($event.target as HTMLSelectElement).value)"
          >
            <option v-for="f in formats" :key="f.key" :value="f.key">
              {{ f.label }}
            </option>
            <option
              v-if="!formats.some((f) => f.key === formatKey)"
              :value="formatKey"
            >
              Custom: {{ formatKey }}
            </option>
          </select>
        </label>
        <div class="export-spec">
          <span
            >Size<strong
              >{{ e.project?.settings.width }} ×
              {{ e.project?.settings.height }}</strong
            ></span
          ><span
            >Length<strong>{{ time(e.duration) }}</strong></span
          ><span>Encoding<strong>CPU · CRF 20</strong></span>
        </div>
        <p class="muted">
          Export uses a snapshot, so you can keep editing. Choose a new file
          name: existing files are never overwritten.
        </p>
        <div class="dialog-actions">
          <button @click="exportDialog = false">Cancel</button
          ><button class="primary" @click="render()">
            Choose file and export
          </button>
        </div>
      </section>
    </div>
  </div>
</template>
