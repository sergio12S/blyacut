import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  Project,
  Edit,
  Clip,
  Snapshot,
  EffectRecipe,
  Settings,
  Transform,
  TransitionKind,
} from "../types";
import { isOverlayTrack, isVideoStep } from "../types";
import { clipLength } from "../timeline-gesture";
export const useEditor = defineStore("editor", () => {
  const project = ref<Project>();
  const selected = ref("");
  const revision = ref(-1);
  const playhead = ref(0);
  const playing = ref(false);
  const error = ref("");
  /** Uncommitted overlay placement while a slider is dragged; the monitor shows it live. */
  const draftTransform = ref<{ clipId: string; transform: Transform }>();
  const busy = ref(false);
  const projectPath = ref("");
  const dirty = ref(false);
  const duration = computed(() =>
    Math.max(
      0,
      ...(project.value?.tracks.flatMap((t) =>
        t.clips.map((c) => c.position_ms + clipLength(c)),
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
  async function edit(operation: Edit, expectedRevision = revision.value) {
    await editBatch([operation], expectedRevision);
  }
  async function editBatch(
    operations: Edit[],
    expectedRevision = revision.value,
  ) {
    playing.value = false;
    await guard(async () => {
      accept(
        await invoke<Snapshot>("edit_batch", {
          operations,
          expectedRevision,
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
            "png",
            "jpg",
            "jpeg",
            "webp",
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
      name: `Звук ${(project.value?.tracks.filter((t) => t.kind === "audio").length ?? 0) + 1}`,
      kind: "audio",
    });
  }
  /** Put an asset on an overlay track at the playhead, creating a track when all are busy there. */
  async function placeOverlay(assetId: string, durationMs?: number) {
    const p = project.value;
    const asset = p?.assets.find((a) => a.id === assetId);
    if (!p || !asset?.width) return;
    const start = Math.round(playhead.value);
    const length = Math.min(
      durationMs ?? (asset.still ? 5000 : asset.duration_ms),
      asset.duration_ms,
    );
    const free = p.tracks.find(
      (t) =>
        isOverlayTrack(p, t) &&
        !t.clips.some(
          (c) =>
            start < c.position_ms + clipLength(c) &&
            start + length > c.position_ms,
        ),
    );
    const trackId = free?.id ?? crypto.randomUUID();
    const clipId = crypto.randomUUID();
    const videoCount = p.tracks.filter((t) => t.kind === "video").length;
    await editBatch([
      ...(free
        ? []
        : [
            {
              operation: "add_track",
              track_id: trackId,
              name: `Видео ${videoCount + 1}`,
              kind: "video",
            } as Edit,
          ]),
      {
        operation: "append",
        asset_id: assetId,
        clip_id: clipId,
        track_id: trackId,
      },
      { operation: "trim_clip", clip_id: clipId, start_ms: 0, end_ms: length },
      {
        operation: "move_clip",
        clip_id: clipId,
        track_id: trackId,
        position_ms: start,
      },
    ]);
    selected.value = clipId;
  }
  /** Transition into a main-track clip from the one before it (null removes it). */
  async function setTransition(
    clipId: string,
    kind: TransitionKind | null,
    durationMs = 800,
  ) {
    const main = project.value?.tracks.find((t) => t.kind === "video");
    const index = main?.clips.findIndex((c) => c.id === clipId) ?? -1;
    if (index < 1) {
      error.value =
        index === 0
          ? "У первого клипа нет предыдущего — переход ставится на второй и следующие клипы"
          : "Переходы ставятся между клипами главной видеодорожки";
      return;
    }
    selected.value = clipId;
    await edit({
      operation: "set_transition",
      clip_id: clipId,
      transition: kind ? { kind, duration_ms: durationMs } : null,
      // Trim the clip edges just enough for the handles instead of failing.
      make_room: true,
    });
    const handles = /needs (\d+) ms of media after .* and (\d+) ms before/.exec(
      error.value,
    );
    if (handles)
      error.value = `Для перехода ${durationMs / 1000} с нужно ${Number(handles[1]) / 1000} с исходника после конца предыдущего клипа и ${Number(handles[2]) / 1000} с до начала этого. Обрежьте клипы (тяните края) или сократите переход.`;
  }
  async function setSpeed(
    clipId: string,
    speedMilli: number,
    reverse: boolean,
  ) {
    await edit({
      operation: "set_speed",
      clip_id: clipId,
      speed_milli: speedMilli,
      reverse,
    });
  }
  /** Hold the frame under the playhead (main track) for `durationMs`. */
  async function freezeFrame(durationMs = 2000) {
    const main = project.value?.tracks.find((t) => t.kind === "video");
    const clip = main?.clips.find(
      (c) =>
        playhead.value >= c.position_ms &&
        playhead.value <= c.position_ms + clipLength(c),
    );
    if (!clip) {
      error.value = "Поставьте плейхед на клип главной видеодорожки";
      return;
    }
    playing.value = false;
    await guard(async () => {
      accept(
        await invoke<Snapshot>("freeze_frame", {
          clipId: clip.id,
          atMs: Math.round(playhead.value),
          durationMs,
        }),
      );
      dirty.value = true;
    });
  }
  async function setSettings(settings: Settings) {
    await edit({ operation: "set_settings", settings });
  }
  async function addVideoTrack() {
    await edit({
      operation: "add_track",
      track_id: crypto.randomUUID(),
      name: `Видео ${(project.value?.tracks.filter((t) => t.kind === "video").length ?? 0) + 1}`,
      kind: "video",
    });
  }
  /** Transparent text image placed over the video at the playhead. */
  async function createTextOverlay(
    text: string,
    textColor: string,
    position: "top" | "center" | "bottom",
    durationMs: number,
    style: "box" | "outline" | "shadow" = "box",
  ) {
    playing.value = false;
    const before = new Set(project.value?.assets.map((a) => a.id));
    await guard(async () => {
      accept(
        await invoke<Snapshot>("create_text_overlay", {
          text,
          textColor,
          position,
          style,
        }),
      );
      dirty.value = true;
    });
    const asset = project.value?.assets.find((a) => !before.has(a.id));
    if (asset) await placeOverlay(asset.id, durationMs);
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
        playhead.value < c.position_ms + clipLength(c),
    );
    if (c)
      await edit({
        operation: "split_clip",
        clip_id: c.id,
        position_ms: Math.round(playhead.value),
        new_clip_id: crypto.randomUUID(),
      });
  }
  /** Append a recipe to a clip's chain; video looks are refused on audio-only tracks. */
  async function applyRecipe(clipId: string, recipe: EffectRecipe) {
    const track = project.value?.tracks.find((t) =>
      t.clips.some((c) => c.id === clipId),
    );
    const clip = track?.clips.find((c) => c.id === clipId);
    if (!track || !clip) return;
    if (track.kind === "audio" && recipe.steps.some(isVideoStep)) {
      error.value = `«${recipe.name}» — видеоэффект, его нельзя применить к звуковой дорожке`;
      return;
    }
    selected.value = clipId;
    await edit({
      operation: "set_effects",
      clip_id: clipId,
      effects: [...clip.effects, recipe],
    });
  }
  /** Render a title card into the app data folder, import it and put it at the end of the video track. */
  async function createTitle(
    text: string,
    textColor: string,
    backgroundColor: string,
    durationMs: number,
  ) {
    playing.value = false;
    const before = new Set(project.value?.assets.map((a) => a.id));
    await guard(async () => {
      accept(
        await invoke<Snapshot>("create_title_card", {
          text,
          textColor,
          backgroundColor,
          durationMs,
        }),
      );
      dirty.value = true;
    });
    // Outside guard: a nested guard would silently skip while busy.
    const asset = project.value?.assets.find((a) => !before.has(a.id));
    if (asset) await append(asset.id);
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
    editBatch,
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
    applyRecipe,
    createTitle,
    placeOverlay,
    setTransition,
    setSettings,
    setSpeed,
    freezeFrame,
    draftTransform,
    addVideoTrack,
    createTextOverlay,
  };
});
