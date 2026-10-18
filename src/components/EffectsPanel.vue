<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEditor } from "../stores/editor";
import type { EffectCatalog, EffectRecipe } from "../types";
const e = useEditor();
const folder = ref("");
const catalog = ref<EffectCatalog>({ recipes: [], warnings: [] });
async function reload() {
  if (!folder.value) return;
  await e.guard(async () => {
    catalog.value = await invoke<EffectCatalog>("load_effect_recipes", {
      root: folder.value,
    });
  });
}
async function choose() {
  const path = await open({
    directory: true,
    multiple: false,
    title: "Choose the folder containing effect recipe JSON files",
  });
  if (typeof path === "string") {
    folder.value = path;
    await reload();
  }
}
function apply(recipe: EffectRecipe) {
  const c = e.selection;
  if (c)
    void e.edit({
      operation: "set_effects",
      clip_id: c.id,
      effects: [...c.effects, recipe],
    });
}
function remove(index: number) {
  const c = e.selection;
  if (c)
    void e.edit({
      operation: "set_effects",
      clip_id: c.id,
      effects: c.effects.filter((_, i) => i !== index),
    });
}
function compatible(recipe: EffectRecipe) {
  return (
    e.selectionTrack?.kind !== "audio" ||
    recipe.steps.every((s) =>
      ["low_pass", "high_pass", "echo"].includes(s.effect),
    )
  );
}
</script>
<template>
  <section class="effects-panel">
    <h4>
      Effect chain
      <span class="badge">{{ e.selection?.effects.length ?? 0 }}</span>
    </h4>
    <div
      v-for="(recipe, index) in e.selection?.effects"
      :key="index"
      class="effect-item"
    >
      <div>
        <strong>{{ recipe.name }}</strong
        ><small>{{
          recipe.steps.map((s) => s.effect.replaceAll("_", " ")).join(" → ")
        }}</small>
      </div>
      <button title="Remove effect recipe" @click="remove(index)">×</button>
    </div>
    <p v-if="!e.selection?.effects.length" class="muted">
      No effects on this clip.
    </p>
    <button class="full" @click="choose()">Load recipe folder…</button
    ><template v-if="folder"
      ><button class="full" @click="reload()">Refresh recipes</button
      ><button
        v-for="recipe in catalog.recipes"
        :key="recipe.id"
        class="full effect-choice"
        :disabled="!compatible(recipe)"
        @click="apply(recipe)"
      >
        ＋ {{ recipe.name }}
      </button>
      <p
        v-for="warning in catalog.warnings"
        :key="warning"
        class="recipe-warning"
      >
        {{ warning }}
      </p></template
    ><small class="muted"
      >JSON recipes reload without reinstall. Applied recipes are saved in this
      project.</small
    >
  </section>
</template>
