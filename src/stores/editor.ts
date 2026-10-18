import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { Project, Edit, Clip, Snapshot } from "../types";
export const useEditor = defineStore("editor", () => {
  const project = ref<Project>();
  const selected = ref("");
  const revision = ref(-1);
  const playhead = ref(0);
  const playing = ref(false);
  const error = ref("");
  const busy = ref(false);
  const projectPath = ref("");
  const dirty = ref(false);
  const duration = computed(() =>
    Math.max(
      0,
      ...(project.value?.tracks.flatMap((t) =>
        t.clips.map((c) => c.position_ms + c.source_end_ms - c.source_start_ms),
      ) ?? []),
    ),
  );
  const clips = computed(
    () => project.value?.tracks.flatMap((t) => t.clips) ?? [],
  );
  const videoClips = computed(
    () =>
      project.value?.tracks
        .filter((t) => t.kind === "video")
        .flatMap((t) => t.clips) ?? [],
  );
  const audioClips = computed(
    () =>
      project.value?.tracks
        .filter((t) => t.kind === "audio")
        .flatMap((t) => t.clips) ?? [],
  );
  const selectionTrack = computed(() =>
    project.value?.tracks.find((t) =>
      t.clips.some((c) => c.id === selected.value),
    ),
  );
  function accept(snapshot: Snapshot, markDirty = true) {
    if (snapshot.revision <= revision.value) return;
    project.value = snapshot.project;
    revision.value = snapshot.revision;
    playing.value = false;
    playhead.value = Math.min(playhead.value, duration.value);
    if (markDirty) dirty.value = true;
  }
  const selection = computed(() =>
    clips.value.find((c) => c.id === selected.value),
  );
  async function guard(action: () => Promise<void>) {
    if (busy.value) return;
    busy.value = true;
    error.value = "";
    try {
      await action();
    } catch (e) {
      error.value = String(e);
    } finally {
      busy.value = false;
    }
  }
  async function refresh() {
    accept(await invoke<Snapshot>("get_project"), false);
  }
  async function edit(operation: Edit) {
    playing.value = false;
    await guard(async () => {
      accept(
        await invoke<Snapshot>("edit", {
          operation,
          expectedRevision: revision.value,
        }),
      );
      dirty.value = true;
      playhead.value = Math.min(playhead.value, duration.value);
    });
  }
  async function importPaths(paths: string[]) {
    playing.value = false;
    await guard(async () => {
      for (const path of paths) {
        accept(await invoke<Snapshot>("import_media", { path }));
        dirty.value = true;
      }
    });
  }
  async function importMedia() {
    const paths = await open({
      multiple: true,
      filters: [
        {
          name: "Media",
          extensions: [
            "mp4",
            "mov",
            "m4v",
            "mkv",
            "webm",
            "wav",
            "mp3",
            "m4a",
            "flac",
            "ogg",
          ],
        },
      ],
    });
    if (paths) await importPaths(Array.isArray(paths) ? paths : [paths]);
  }
  async function addAudioTrack() {
    await edit({
      operation: "add_track",
      track_id: crypto.randomUUID(),
      name: `Audio ${(project.value?.tracks.filter((t) => t.kind === "audio").length ?? 0) + 1}`,
      kind: "audio",
    });
  }
  async function append(assetId: string, kind?: "video" | "audio") {
    const asset = project.value?.assets.find((a) => a.id === assetId);
    if (!asset) return;
    kind = kind ?? (asset.width > 0 ? "video" : "audio");
    if (
      kind === "audio" &&
      !project.value?.tracks.some((t) => t.kind === "audio")
    )
      await addAudioTrack();
    const track = project.value?.tracks.find((t) => t.kind === kind);
    if (!track) return;
    const cid = crypto.randomUUID();
    await edit({
      operation: "append",
      asset_id: assetId,
      clip_id: cid,
      track_id: track.id,
    });
    selected.value = cid;
  }
  async function split() {
    const c = (selectionTrack.value?.clips ?? videoClips.value).find(
      (c) =>
        playhead.value > c.position_ms &&
        playhead.value < c.position_ms + c.source_end_ms - c.source_start_ms,
    );
    if (c)
      await edit({
        operation: "split_clip",
        clip_id: c.id,
        position_ms: Math.round(playhead.value),
        new_clip_id: crypto.randomUUID(),
      });
  }
  async function remove() {
    if (selection.value)
      await edit({ operation: "delete_clip", clip_id: selected.value });
    selected.value = "";
  }
  async function history(redo = false) {
    playing.value = false;
    await guard(async () => {
      accept(
        await invoke<Snapshot>("history", {
          redo,
          expectedRevision: revision.value,
        }),
      );
      dirty.value = true;
      playhead.value = Math.min(playhead.value, duration.value);
    });
  }
  async function saveProject(as = false) {
    await guard(async () => {
      const path =
        (!as && projectPath.value) ||
        (await save({
          defaultPath: "Untitled.blyacut",
          filters: [{ name: "BlyaCut project", extensions: ["blyacut"] }],
        }));
      if (path) {
        const savedRevision = await invoke<number>("save_project", { path });
        projectPath.value = path;
        dirty.value = revision.value !== savedRevision;
      }
    });
  }
  async function openProject() {
    playing.value = false;
    await guard(async () => {
      const path = await open({
        multiple: false,
        filters: [{ name: "BlyaCut project", extensions: ["blyacut"] }],
      });
      if (typeof path === "string") {
        accept(await invoke<Snapshot>("open_project", { path }), false);
        projectPath.value = path;
        playhead.value = 0;
        selected.value = "";
        dirty.value = false;
        const missing = await invoke<string[]>("missing_media");
        if (missing.length)
          error.value = `Missing media: ${missing.join(", ")}`;
      }
    });
  }
  function seek(ms: number) {
    playhead.value = Math.round(Math.max(0, Math.min(duration.value, ms)));
  }
  function step(direction: number) {
    playing.value = false;
    const s = project.value?.settings;
    if (!s) return;
    const frame = Math.round((playhead.value * s.fps_num) / (1000 * s.fps_den));
    seek(((frame + direction) * 1000 * s.fps_den) / s.fps_num);
  }
  function select(c: Clip) {
    selected.value = c.id;
    playing.value = false;
    seek(c.position_ms);
  }
  return {
    project,
    revision,
    accept,
    videoClips,
    audioClips,
    selectionTrack,
    addAudioTrack,
    selected,
    playhead,
    playing,
    error,
    busy,
    projectPath,
    dirty,
    duration,
    clips,
    selection,
    guard,
    refresh,
    edit,
    importPaths,
    importMedia,
    append,
    split,
    remove,
    history,
    saveProject,
    openProject,
    seek,
    step,
    select,
  };
});
