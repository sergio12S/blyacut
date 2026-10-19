<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEditor } from "../stores/editor";
import type { EffectCatalog, EffectRecipe, EffectStep } from "../types";
import { RECIPE_MIME } from "../types";
import {
  EFFECTS_CATALOG,
  formatEffectStep,
  formatRecipeSteps,
  getEffectMeta,
  isAudioCompatibleRecipe,
} from "../effectsCatalog";

const e = useEditor();
/** "library": preset gallery and custom builder; "chain": the selected clip's effect stack. */
const props = withDefaults(defineProps<{ section?: "chain" | "library" }>(), {
  section: "chain",
});
function dragRecipe(event: DragEvent, recipe: EffectRecipe) {
  event.dataTransfer?.setData(RECIPE_MIME, JSON.stringify(recipe));
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "copy";
}

// Single source of truth: `blyacut_core::effects::builtin_recipes` (Rust).
const builtinRecipes = ref<EffectRecipe[]>([]);

// Built-in presets come from Rust (`builtin_effect_recipes`).
onMounted(async () => {
  if (!isTauri()) return;
  try {
    const list = await invoke<EffectRecipe[]>("builtin_effect_recipes");
    if (Array.isArray(list) && list.length > 0) {
      builtinRecipes.value = list;
    }
  } catch (error) {
    e.error = `Could not load built-in effects: ${error}`;
  }
});

// --- Preset gallery tabs ---
const activeTab = ref<"video" | "audio">("video");
const searchQuery = ref("");

// An audio-track selection switches the gallery to audio effects
watch(
  () => e.selectionTrack?.kind,
  (kind) => {
    if (kind === "audio") {
      activeTab.value = "audio";
    }
  },
);

const filteredRecipes = computed(() => {
  const query = searchQuery.value.trim().toLowerCase();
  return builtinRecipes.value.filter((recipe) => {
    const isAudio = isAudioCompatibleRecipe(recipe);
    if (activeTab.value === "video" && isAudio) return false;
    if (activeTab.value === "audio" && !isAudio) return false;

    if (!query) return true;
    const nameMatch = recipe.name.toLowerCase().includes(query);
    const stepMatch = recipe.steps.some(
      (s) =>
        s.effect.toLowerCase().includes(query) ||
        (getEffectMeta(s.effect)?.name.toLowerCase().includes(query) ?? false),
    );
    return nameMatch || stepMatch;
  });
});

// --- Selection and limits ---
const currentClip = computed(() => e.selection);
const isAudioTrack = computed(() => e.selectionTrack?.kind === "audio");
const currentEffects = computed(() => currentClip.value?.effects ?? []);
const totalSteps = computed(() =>
  currentEffects.value.reduce((acc, r) => acc + r.steps.length, 0),
);

const canAddMoreRecipes = computed(() => {
  if (!currentClip.value) return false;
  return currentEffects.value.length < 8 && totalSteps.value < 32;
});

function getRecipeIncompatibilityReason(recipe: EffectRecipe): string | null {
  if (!currentClip.value) return "Select a clip on the timeline";
  if (currentEffects.value.length >= 8)
    return "Limit reached: at most 8 effects per clip";
  if (totalSteps.value + recipe.steps.length > 32)
    return "Limit reached: at most 32 steps per clip";
  if (isAudioTrack.value && !isAudioCompatibleRecipe(recipe)) {
    return "Video effects cannot go on an audio track";
  }
  return null;
}

// --- The clip's effect chain ---
function applyRecipe(recipe: EffectRecipe) {
  const c = currentClip.value;
  if (!c || getRecipeIncompatibilityReason(recipe)) return;
  void e.applyRecipe(c.id, recipe);
}

function removeRecipe(index: number) {
  const c = currentClip.value;
  if (!c) return;

  void e.edit({
    operation: "set_effects",
    clip_id: c.id,
    effects: c.effects.filter((_, i) => i !== index),
  });
}

function moveRecipe(index: number, direction: -1 | 1) {
  const c = currentClip.value;
  if (!c) return;
  const targetIndex = index + direction;
  if (targetIndex < 0 || targetIndex >= c.effects.length) return;

  const next = [...c.effects];
  const [item] = next.splice(index, 1);
  next.splice(targetIndex, 0, item);

  void e.edit({
    operation: "set_effects",
    clip_id: c.id,
    effects: next,
  });
}

// --- Custom single-step effect with sliders ---
const customStepEffect = ref<EffectStep["effect"]>("contrast");
const customParamValues = ref<Record<string, number>>({});
const customRecipeName = ref("");

// Reset parameters to defaults when the step type changes
function onCustomStepChange() {
  const meta = getEffectMeta(customStepEffect.value);
  if (!meta) return;
  const vals: Record<string, number> = {};
  for (const p of meta.params) {
    vals[p.key] = p.default;
  }
  customParamValues.value = vals;
  customRecipeName.value = meta.name;
}

onMounted(() => onCustomStepChange());

const selectedStepMeta = computed(() => getEffectMeta(customStepEffect.value));

function addCustomEffect() {
  const c = currentClip.value;
  const meta = selectedStepMeta.value;
  if (!c || !meta) return;

  let step: EffectStep;
  if (meta.params.length === 0) {
    step = { effect: meta.effect } as EffectStep;
  } else {
    step = {
      effect: meta.effect,
      ...customParamValues.value,
    } as unknown as EffectStep;
  }

  const recipe: EffectRecipe = {
    version: 1,
    id: `custom-${Date.now()}`,
    name: customRecipeName.value.trim() || meta.name,
    steps: [step],
  };

  applyRecipe(recipe);
}

// --- Recipe folder on disk ---
const folder = ref("");
const folderCatalog = ref<EffectCatalog>({ recipes: [], warnings: [] });

async function chooseFolder() {
  const path = await open({
    directory: true,
    multiple: false,
    title: "Choose a folder of JSON effect recipes",
  });
  if (typeof path === "string") {
    folder.value = path;
    await reloadFolder();
  }
}

async function reloadFolder() {
  if (!folder.value) return;
  await e.guard(async () => {
    folderCatalog.value = await invoke<EffectCatalog>("load_effect_recipes", {
      root: folder.value,
    });
  });
}
</script>

<template>
  <section class="effects-panel">
    <!-- Header -->
    <div v-if="props.section === 'chain'" class="panel-header-row">
      <div class="header-left">
        <h4 class="title">
          Effects
          <span class="badge">{{ currentEffects.length }}</span>
        </h4>
        <span v-if="currentClip" class="clip-sub">
          {{ isAudioTrack ? "Audio" : "Video" }} track clip
        </span>
      </div>
      <div
        v-if="currentClip"
        class="steps-count"
        :class="{ warn: totalSteps >= 28 }"
      >
        {{ currentEffects.length }}/8 effects · {{ totalSteps }}/32 steps
      </div>
    </div>

    <!-- 1. The clip's effect chain -->
    <div v-if="props.section === 'chain'" class="section-block">
      <div class="section-title">Effect chain</div>

      <div v-if="!currentClip" class="empty-hint">
        <span>👆 Select a clip on the timeline to add or adjust effects.</span>
      </div>

      <div v-else-if="!currentEffects.length" class="empty-hint">
        <span
          >No effects on this clip yet. Drag one from the library or build your
          own.</span
        >
      </div>

      <div v-else class="chain-list">
        <div
          v-for="(recipe, index) in currentEffects"
          :key="recipe.id + '-' + index"
          class="chain-item"
        >
          <div class="chain-info">
            <strong class="chain-name">{{ recipe.name }}</strong>
            <small class="chain-steps">{{ formatRecipeSteps(recipe) }}</small>
          </div>
          <div class="chain-controls">
            <button
              class="icon-btn"
              title="Move up"
              :disabled="index === 0"
              @click="moveRecipe(index, -1)"
            >
              ↑
            </button>
            <button
              class="icon-btn"
              title="Move down"
              :disabled="index === currentEffects.length - 1"
              @click="moveRecipe(index, 1)"
            >
              ↓
            </button>
            <button
              class="icon-btn danger"
              title="Remove effect"
              @click="removeRecipe(index)"
            >
              ×
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- 2. Built-in preset gallery -->
    <div v-if="props.section === 'library'" class="section-block">
      <div class="section-title-row">
        <div class="section-title">Presets</div>
        <div class="tabs-group">
          <button
            class="tab-btn"
            :class="{ active: activeTab === 'video' }"
            :disabled="isAudioTrack"
            title="Video effects"
            @click="activeTab = 'video'"
          >
            Video
          </button>
          <button
            class="tab-btn"
            :class="{ active: activeTab === 'audio' }"
            title="Audio effects"
            @click="activeTab = 'audio'"
          >
            Audio
          </button>
        </div>
      </div>

      <!-- Search -->
      <div class="search-box">
        <input
          v-model="searchQuery"
          type="text"
          placeholder="Search presets (name, step)…"
          class="search-input"
        />
        <button
          v-if="searchQuery"
          class="clear-search"
          @click="searchQuery = ''"
        >
          ×
        </button>
      </div>

      <!-- Preset cards (2 columns) -->
      <div class="presets-grid">
        <div
          v-for="recipe in filteredRecipes"
          :key="recipe.id"
          class="preset-card"
          :class="{
            disabled: !!currentClip && !!getRecipeIncompatibilityReason(recipe),
          }"
          :title="
            currentClip
              ? (getRecipeIncompatibilityReason(recipe) ??
                'Click or drag onto a clip')
              : 'Drag onto a clip on the timeline'
          "
          tabindex="0"
          draggable="true"
          @dragstart="dragRecipe($event, recipe)"
          @click="applyRecipe(recipe)"
          @keydown.enter="applyRecipe(recipe)"
        >
          <div class="preset-name-row">
            <strong class="preset-title">{{ recipe.name }}</strong>
            <span class="preset-add-badge">＋</span>
          </div>
          <small class="preset-steps-summary">
            {{ formatRecipeSteps(recipe) }}
          </small>
        </div>
      </div>

      <p v-if="filteredRecipes.length === 0" class="muted-centered">
        Nothing matches “{{ searchQuery }}”.
      </p>
    </div>

    <!-- 3. Custom effect builder -->
    <details
      v-if="props.section === 'library'"
      class="section-block custom-effect-details"
    >
      <summary class="section-title summary-toggle">
        <span>Build your own effect</span>
      </summary>

      <div class="custom-builder-body">
        <div class="field-row">
          <label class="field-label">Step</label>
          <select
            v-model="customStepEffect"
            class="select-input"
            @change="onCustomStepChange"
          >
            <option
              v-for="meta in EFFECTS_CATALOG"
              :key="meta.effect"
              :value="meta.effect"
              :disabled="isAudioTrack && meta.kind === 'video'"
            >
              {{ meta.icon }} {{ meta.name }} ({{
                meta.kind === "video" ? "Video" : "Audio"
              }})
            </option>
          </select>
        </div>

        <div v-if="selectedStepMeta" class="step-desc-hint">
          {{ selectedStepMeta.description }}
        </div>

        <!-- Parameter sliders -->
        <div v-if="selectedStepMeta?.params.length" class="params-list">
          <div
            v-for="p in selectedStepMeta.params"
            :key="p.key"
            class="param-row"
          >
            <div class="param-header">
              <span>{{ p.name }}</span>
              <span class="param-val">
                {{
                  p.format
                    ? p.format(customParamValues[p.key] ?? p.default)
                    : (customParamValues[p.key] ?? p.default) + p.unit
                }}
              </span>
            </div>
            <input
              v-model.number="customParamValues[p.key]"
              type="range"
              :min="p.min"
              :max="p.max"
              :step="p.step"
              class="slider"
            />
          </div>
        </div>

        <div class="field-row">
          <label class="field-label">Effect name</label>
          <input
            v-model="customRecipeName"
            type="text"
            placeholder="Preset name"
            class="text-input"
          />
        </div>

        <button
          class="primary full"
          :disabled="
            !canAddMoreRecipes ||
            (isAudioTrack && selectedStepMeta?.kind === 'video')
          "
          @click="addCustomEffect"
        >
          ＋ Add effect to clip
        </button>
      </div>
    </details>

    <!-- 4. Recipe folder on disk -->
    <details
      v-if="props.section === 'library'"
      class="section-block folder-details"
    >
      <summary class="section-title summary-toggle">
        <span>Your JSON recipe folder</span>
      </summary>

      <div class="folder-body">
        <button class="full" @click="chooseFolder">
          {{ folder ? "Change recipe folder…" : "Load recipe folder…" }}
        </button>

        <template v-if="folder">
          <div class="folder-path-label">{{ folder }}</div>
          <button class="full secondary" @click="reloadFolder">
            Reload recipes from folder
          </button>

          <div v-if="folderCatalog.recipes.length" class="folder-recipes-list">
            <button
              v-for="recipe in folderCatalog.recipes"
              :key="recipe.id"
              class="full effect-choice"
              :disabled="
                !canAddMoreRecipes || !!getRecipeIncompatibilityReason(recipe)
              "
              @click="applyRecipe(recipe)"
            >
              ＋ {{ recipe.name }}
            </button>
          </div>

          <p
            v-for="warning in folderCatalog.warnings"
            :key="warning"
            class="recipe-warning"
          >
            ⚠️ {{ warning }}
          </p>
        </template>
      </div>
    </details>

    <!-- Honest note on preview vs export -->
    <div class="footer-disclaimer">
      💡 Export applies every effect. The monitor preview is approximate (color,
      blur, black & white, sepia, negative, mirror only).
    </div>
  </section>
</template>

<style scoped>
.effects-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  color: var(--text-main, #d9dce5);
  font-size: 12px;
}

.panel-header-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  padding-bottom: 8px;
  border-bottom: 1px solid #2b2d36;
}

.title {
  margin: 0;
  font-size: 13px;
  font-weight: 700;
  display: flex;
  align-items: center;
  gap: 6px;
  color: #fff;
}

.badge {
  background: #252834;
  color: #c1f17b;
  font-size: 10px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 10px;
  border: 1px solid #363a4c;
}

.clip-sub {
  font-size: 10px;
  color: #888d9e;
}

.steps-count {
  font-size: 10px;
  font-family: ui-monospace, monospace;
  color: #888d9e;
  background: #161820;
  padding: 2px 6px;
  border-radius: 4px;
  border: 1px solid #2b2d36;
}

.steps-count.warn {
  color: #f59e0b;
  border-color: #f59e0b;
}

.section-block {
  background: #14161d;
  border: 1px solid #272a36;
  border-radius: 6px;
  padding: 10px;
}

.section-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.section-title {
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  color: #9da3b4;
  margin-bottom: 6px;
}

.summary-toggle {
  cursor: pointer;
  user-select: none;
  margin-bottom: 0;
}

.tabs-group {
  display: flex;
  gap: 4px;
  background: #0d0e13;
  padding: 2px;
  border-radius: 4px;
  border: 1px solid #2b2d36;
}

.tab-btn {
  background: transparent;
  border: none;
  color: #888d9e;
  padding: 3px 8px;
  font-size: 11px;
  font-weight: 600;
  border-radius: 3px;
  cursor: pointer;
}

.tab-btn.active {
  background: #23252d;
  color: #c1f17b;
}

.tab-btn:disabled {
  opacity: 0.35;
  cursor: default;
}

.empty-hint {
  font-size: 11px;
  color: #727787;
  padding: 6px 2px;
  line-height: 1.4;
}

/* Effect chain */
.chain-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.chain-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: #1b1e28;
  border: 1px solid #2c303f;
  padding: 6px 8px;
  border-radius: 4px;
}

.chain-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.chain-name {
  font-size: 12px;
  color: #eff1f6;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.chain-steps {
  font-size: 10px;
  color: #888d9e;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-top: 1px;
}

.chain-controls {
  display: flex;
  gap: 4px;
  margin-left: 8px;
}

.icon-btn {
  background: #252834;
  border: 1px solid #363944;
  color: #dfe1eb;
  padding: 2px 6px;
  font-size: 11px;
  border-radius: 3px;
  cursor: pointer;
}

.icon-btn:hover:not(:disabled) {
  background: #343746;
}

.icon-btn.danger:hover:not(:disabled) {
  background: rgba(239, 68, 68, 0.2);
  color: #f87171;
  border-color: #f87171;
}

.icon-btn:disabled {
  opacity: 0.3;
  cursor: default;
}

/* Search */
.search-box {
  position: relative;
  margin-bottom: 8px;
}

.search-input {
  width: 100%;
  background: #0d0f14;
  border: 1px solid #2b2e3c;
  color: #eff1f6;
  font-size: 11px;
  padding: 5px 24px 5px 8px;
  border-radius: 4px;
  outline: none;
}

.search-input:focus {
  border-color: #c1f17b;
}

.clear-search {
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  background: none;
  border: none;
  color: #888d9e;
  font-size: 13px;
  cursor: pointer;
  padding: 0;
}

/* Preset grid */
.presets-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
  max-height: 260px;
  overflow-y: auto;
  padding-right: 2px;
}

.preset-card {
  background: #1a1c24;
  border: 1px solid #2b2e3c;
  border-radius: 5px;
  padding: 7px 8px;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  transition: all 0.15s ease;
}

.preset-card:hover:not(.disabled) {
  background: #232632;
  border-color: #c1f17b;
}

.preset-card.disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.preset-name-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 3px;
}

.preset-title {
  font-size: 11px;
  font-weight: 600;
  color: #e5e8f0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.preset-add-badge {
  font-size: 11px;
  color: #c1f17b;
  font-weight: 700;
}

.preset-steps-summary {
  font-size: 9.5px;
  color: #888d9e;
  line-height: 1.3;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.muted-centered {
  text-align: center;
  font-size: 11px;
  color: #727787;
  padding: 16px 0;
}

/* Effect builder */
.custom-effect-details summary {
  margin-bottom: 0;
}

.custom-builder-body {
  padding-top: 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.field-row {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.field-label {
  font-size: 10px;
  color: #888d9e;
  font-weight: 600;
}

.select-input,
.text-input {
  width: 100%;
  background: #0d0f14;
  border: 1px solid #2b2e3c;
  color: #eff1f6;
  font-size: 11px;
  padding: 6px;
  border-radius: 4px;
}

.select-input:focus,
.text-input:focus {
  border-color: #c1f17b;
  outline: none;
}

.step-desc-hint {
  font-size: 10px;
  color: #888d9e;
  font-style: italic;
}

.params-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: #0d0f14;
  padding: 8px;
  border-radius: 4px;
  border: 1px solid #232530;
}

.param-header {
  display: flex;
  justify-content: space-between;
  font-size: 10.5px;
  color: #c5c9d6;
  margin-bottom: 2px;
}

.param-val {
  font-family: ui-monospace, monospace;
  color: #c1f17b;
}

.slider {
  width: 100%;
  height: 4px;
  background: #252834;
  border-radius: 2px;
  outline: none;
  -webkit-appearance: none;
}

.slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #c1f17b;
  cursor: pointer;
}

/* Recipe folder */
.folder-details summary {
  margin-bottom: 0;
}

.folder-body {
  padding-top: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.folder-path-label {
  font-family: ui-monospace, monospace;
  font-size: 10px;
  color: #888d9e;
  background: #0d0f14;
  padding: 4px 6px;
  border-radius: 3px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.folder-recipes-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 120px;
  overflow-y: auto;
}

.recipe-warning {
  color: #f87171;
  font-size: 10px;
  margin: 0;
}

.footer-disclaimer {
  font-size: 10.5px;
  color: #7b8192;
  line-height: 1.4;
  padding: 6px 8px;
  background: #0d0e13;
  border-radius: 4px;
  border: 1px solid #1f212a;
}
</style>
