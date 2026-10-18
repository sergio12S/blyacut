// Node's built-in assertions and the existing TypeScript compiler; no test dependency.
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { execFileSync } from "node:child_process";
const out = mkdtempSync(join(tmpdir(), "blyacut-gesture-"));
try {
  execFileSync(
    process.execPath,
    [
      "node_modules/typescript/bin/tsc",
      "--target",
      "ES2022",
      "--module",
      "ES2022",
      "--moduleResolution",
      "bundler",
      "--strict",
      "--skipLibCheck",
      "--outDir",
      out,
      "src/timeline-gesture.ts",
    ],
    { stdio: "inherit" },
  );
  writeFileSync(join(out, "package.json"), '{"type":"module"}');
  const { planGesture, snapPosition, frameTime, clipHitMode } = await import(
    pathToFileURL(join(out, "timeline-gesture.js"))
  );
  const clip = (id, position, start, end) => ({
    id,
    asset_id: "media",
    position_ms: position,
    source_start_ms: start,
    source_end_ms: end,
    gain_milli: 1000,
    fade_in_ms: 0,
    fade_out_ms: 0,
    effects: [],
  });
  const project = {
    version: 3,
    id: "project",
    name: "test",
    settings: { width: 640, height: 360, fps_num: 30, fps_den: 1 },
    assets: [
      {
        id: "media",
        name: "test.mp4",
        path: "/tmp/test.mp4",
        duration_ms: 10000,
        width: 640,
        height: 360,
        has_audio: true,
      },
    ],
    tracks: [
      {
        id: "v",
        name: "Video",
        kind: "video",
        clips: [clip("v1", 0, 0, 2000), clip("v2", 2000, 4000, 7000)],
      },
      {
        id: "a",
        name: "Audio",
        kind: "audio",
        clips: [clip("a1", 1000, 1000, 3000), clip("a2", 5000, 0, 2000)],
      },
      { id: "b", name: "Audio 2", kind: "audio", clips: [] },
    ],
  };
  const original = structuredClone(project);
  const opts = {
    magnetic: false,
    snap: false,
    pixelsPerSecond: 100,
    playhead: 0,
  };
  const plan = (id, mode, delta, track, extra = {}) =>
    planGesture(project, id, mode, delta, track, { ...opts, ...extra });
  let count = 0;
  const test = (name, run) => {
    run();
    count++;
    console.log(`✓ ${name}`);
  };
  test("Border and resize-handle hits trim; clip centre remains draggable", () => {
    assert.equal(clipHitMode(0, 130), "trim-start");
    assert.equal(clipHitMode(129, 130), "trim-end");
    assert.equal(clipHitMode(65, 130), "move");
    assert.equal(clipHitMode(2, 4), "move");
  });
  test("Free video move leaves a precise gap without changing neighbours", () => {
    const p = plan("v2", "move", 2000, "v");
    assert.deepEqual(p.edits, [
      {
        operation: "move_clip",
        clip_id: "v2",
        track_id: "v",
        position_ms: 4000,
      },
    ]);
    assert.equal(p.placements.v1, undefined);
    assert.equal(p.error, undefined);
  });
  test("Magnetic reorder previews unequal-length clips and emits one command", () => {
    const p = plan("v2", "move", -3000, "v", { magnetic: true });
    assert.deepEqual(p.edits, [
      { operation: "reorder_clip", clip_id: "v2", index: 0 },
    ]);
    assert.equal(p.placements.v2.position, 0);
    assert.equal(p.placements.v1.position, 3000);
  });
  test("No-op insert does not create history", () =>
    assert.equal(
      plan("v2", "move", 100, "v", { magnetic: true }).edits.length,
      0,
    ));
  test("Same-track overlap is explained before drop", () =>
    assert.match(plan("v2", "move", -1000, "v").error, /overlap/));
  test("Audio moves between tracks without rippling its source", () => {
    const p = plan("a1", "move", 2500, "b");
    assert.equal(p.edits[0].track_id, "b");
    assert.equal(p.placements.a1.position, 3500);
    assert.equal(p.placements.a2, undefined);
  });
  test("Cross-kind and out-of-lane drops are rejected", () => {
    assert.ok(plan("a1", "move", 0, "v").error);
    assert.ok(plan("a1", "move", 0, "").error);
  });
  test("Snapping includes trailing edges and a screen-space threshold", () => {
    assert.deepEqual(snapPosition(1970, 2000, [4000], 80), {
      position: 2000,
      guide: 4000,
    });
    assert.equal(snapPosition(1900, 2000, [4000], 80).position, 1900);
    const p = plan("v2", "move", 1980, "v", { snap: true, playhead: 4000 });
    assert.equal(p.placements.v2.position, 4000);
    assert.equal(p.guide, 4000);
  });
  test("Left audio shrink preserves right edge and trims before moving", () => {
    const p = plan("a1", "trim-start", 1000, "a");
    assert.equal(p.placements.a1.position, 2000);
    assert.equal(p.placements.a1.start, 2000);
    assert.equal(
      p.placements.a1.position + p.placements.a1.end - p.placements.a1.start,
      3000,
    );
    assert.deepEqual(
      p.edits.map((e) => e.operation),
      ["trim_clip", "move_clip"],
    );
  });
  test("Left audio extension moves before trimming to avoid transient collision", () => {
    const p = plan("a1", "trim-start", -500, "a");
    assert.equal(p.placements.a1.position, 500);
    assert.equal(p.placements.a1.start, 500);
    assert.deepEqual(
      p.edits.map((e) => e.operation),
      ["move_clip", "trim_clip"],
    );
  });
  test("Video edge trim previews ripple, including following clips", () => {
    const p = plan("v1", "trim-end", -1000, "v");
    assert.equal(p.placements.v1.end, 1000);
    assert.equal(p.placements.v2.position, 1000);
    assert.equal(p.error, undefined);
  });
  test("Trim cannot extend past source duration or before source zero", () => {
    assert.equal(plan("v2", "trim-end", 50000, "v").placements.v2.end, 10000);
    assert.equal(plan("v2", "trim-start", -50000, "v").placements.v2.start, 0);
    assert.equal(
      plan("a1", "trim-start", -50000, "a").placements.a1.position,
      0,
    );
  });
  test("Rational frame alignment stays integer without cumulative increments", () => {
    assert.equal(frameTime(1000, { fps_num: 30000, fps_den: 1001 }), 1001);
    assert.equal(frameTime(0, project.settings), 0);
  });
  test("Plans never mutate the authoritative project snapshot", () =>
    assert.deepEqual(project, original));
  console.log(`${count} timeline gesture checks passed`);
} finally {
  rmSync(out, { recursive: true, force: true });
}
