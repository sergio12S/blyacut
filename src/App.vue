<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
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
import type { Snapshot } from "./types";
import { time } from "./types";
const e = useEditor();
const start = ref(0),
  end = ref(0);
const exporting = ref(false),
  progress = ref(0),
  exportStatus = ref("");
const exportDialog = ref(false);
const cleanup: UnlistenFn[] = [];
let autosaveTimer: ReturnType<typeof setInterval> | undefined;
const autosavePath = ref("");
const dock = ref<'inspector' | 'agent'>('inspector');
const pendingEdits = ref(0);
function dockKey(event: KeyboardEvent) {
  if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
  event.preventDefault();
  event.stopPropagation();
  dock.value = event.key === "Home" ? "inspector" : event.key === "End" ? "agent" : dock.value === "agent" ? "inspector" : "agent";
  document.getElementById(`${dock.value}-tab`)?.focus();
}
watch(
  () => e.selection,
  (c) => {
    start.value = c?.source_start_ms ?? 0;
    end.value = c?.source_end_ms ?? 0;
  },
);
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
      "Run npm run tauri dev to use the desktop editor. Browser mode has no filesystem or editing backend.";
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
          (p.status === "complete" ? "Export complete" : "Rendering…");
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
    exportStatus.value = "Rendering…";
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
      <div class="brand">
        <span class="brand-icon">B/</span
        ><span>BlyaCut<span class="brand-tag">LOCAL FIRST</span></span>
      </div>
      <div class="project-name">
        {{ e.project?.name ?? "Untitled project"
        }}<span v-if="e.dirty" class="dirty"> ●</span
        ><span class="badge">MVP 0.2</span>
      </div>
      <nav>
        <button :disabled="e.busy" @click="e.openProject()">Open</button
        ><button :disabled="!e.project || e.busy" @click="e.saveProject()">
          Save <kbd>⌘S</kbd></button
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
      {{ e.error }}<button @click="e.error = ''">×</button>
    </div>
    <div class="workspace">
      <aside class="media-panel">
        <div class="panel-title">
          <span>MEDIA LIBRARY</span
          ><span>{{ e.project?.assets.length ?? 0 }}</span>
        </div>
        <div class="media-actions">
          <button
            class="import-button"
            :disabled="e.busy"
            @click="e.importMedia()"
          >
            ＋ Import media
          </button>
          <p>Drop video or audio files anywhere</p>
        </div>
        <div class="media-list">
          <div
            v-for="asset in e.project?.assets"
            :key="asset.id"
            class="media-item"
          >
            <div class="media-thumbnail">
              ▷<span>{{ time(asset.duration_ms).slice(0, 8) }}</span>
            </div>
            <div class="media-meta">
              <strong :title="asset.path">{{ asset.name }}</strong
              ><small
                >{{
                  asset.width
                    ? `${asset.width} × ${asset.height}`
                    : "Audio file"
                }}
                · {{ asset.has_audio ? "Audio" : "Silent" }}</small
              >
            </div>
            <button
              class="add-media"
              :disabled="e.busy"
              title="Append to timeline"
              @click="e.append(asset.id)"
            >
              ＋
            </button>
          </div>
          <div v-if="!e.project?.assets.length" class="library-empty">
            <span>▱</span>
            <p>A place for your footage</p>
            <small>Video · WAV · MP3 · M4A · FLAC</small>
          </div>
        </div>
        <div class="library-footer">● Original files stay untouched</div>
      </aside>
      <Preview />
      <aside class="inspector">
        <div class="dock-tabs" role="tablist" aria-label="Editor panels" @keydown="dockKey">
          <button id="inspector-tab" role="tab" :tabindex="dock === 'inspector' ? 0 : -1" :aria-selected="dock === 'inspector'" aria-controls="inspector-panel" @click="dock = 'inspector'">Inspector</button>
          <button id="agent-tab" role="tab" :tabindex="dock === 'agent' ? 0 : -1" :aria-selected="dock === 'agent'" aria-controls="agent-panel" @click="dock = 'agent'">Agent <span v-if="pendingEdits" class="pending-count">{{ pendingEdits }}</span></button>
        </div>
        <div id="inspector-panel" v-show="dock === 'inspector'" class="dock-content" role="tabpanel" aria-labelledby="inspector-tab">
        <div v-if="e.selection" class="inspector-body">
          <h3>
            {{
              e.project?.assets.find((a) => a.id === e.selection?.asset_id)
                ?.name
            }}
          </h3>
          <div class="paired-fields"><label
            >Source in <small>milliseconds</small
            ><input
              v-model.number="start"
              type="number"
              min="0"
              step="1" /></label
          ><label
            >Source out <small>milliseconds</small
            ><input
              v-model.number="end"
              type="number"
              min="1"
              step="1" /></label></div
          ><button
            class="primary full"
            @click="
              e.edit({
                operation: 'trim_clip',
                clip_id: e.selected,
                start_ms: start,
                end_ms: end,
              })
            "
          >
            Apply trim
          </button>
          <dl>
            <dt>Timeline position</dt>
            <dd>{{ time(e.selection.position_ms) }}</dd>
            <dt>Duration</dt>
            <dd>
              {{
                time(e.selection.source_end_ms - e.selection.source_start_ms)
              }}
            </dd>
          </dl>
          <details class="inspector-section"><summary>Audio & timeline position</summary><ClipAudioControls /></details>
          <EffectsPanel />
          <button class="full" @click="e.split()">Split at playhead</button
          ><button class="full danger" @click="e.remove()">
            Remove selected clip
          </button>
        </div>
        <div v-else class="inspector-empty">
          Select a clip to adjust its source range.
        </div>
        </div>
        <div id="agent-panel" v-show="dock === 'agent'" class="dock-content" role="tabpanel" aria-labelledby="agent-tab">
          <AgentPanel @pending="pendingEdits = $event" />
        </div>
      </aside>
    </div>
    <div class="history-bar">
      <button @click="e.history()">↶ Undo</button
      ><button @click="e.history(true)">↷ Redo</button
      ><span v-if="e.busy">Working…</span
      ><span class="history-right"
        >{{ exportStatus }}
        <template v-if="exporting"
          >{{ Math.round(progress * 100) }}%
          <button @click="invoke('cancel_export')">Cancel</button></template
        ></span
      >
    </div>
    <div v-if="exporting" class="progress-track">
      <div :style="{ width: progress * 100 + '%' }" />
    </div>
    <Timeline />
    <footer class="status-bar">
      <span><i /> LOCAL WORKSPACE</span
      ><span :title="autosavePath">{{
        autosavePath
          ? "Recovery snapshot saved"
          : "Rust core · Non-destructive editing"
      }}</span
      ><span>{{ e.projectPath || "Project not saved" }}</span>
    </footer>
    <div
      v-if="exportDialog"
      class="modal-backdrop"
      @click.self="exportDialog = false"
    >
      <section class="export-dialog">
        <span class="eyebrow">READY TO SHARE</span>
        <h2>Export your timeline</h2>
        <p>H.264 video · AAC audio · MP4</p>
        <div class="export-spec">
          <span
            >Resolution<strong
              >{{ e.project?.settings.width }} ×
              {{ e.project?.settings.height }}</strong
            ></span
          ><span
            >Duration<strong>{{ time(e.duration) }}</strong></span
          ><span>Encoding<strong>CPU · veryfast / CRF 20</strong></span>
        </div>
        <p class="muted">
          Export uses a snapshot of your timeline. Choose a new filename;
          existing files are never overwritten. Progress follows FFmpeg
          encoding.
        </p>
        <div class="dialog-actions">
          <button @click="exportDialog = false">Cancel</button
          ><button class="primary" @click="render()">
            Choose destination & export
          </button>
        </div>
      </section>
    </div>
  </div>
</template>
