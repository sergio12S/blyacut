<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEditor } from "../stores/editor";
import type { EffectCatalog, EffectRecipe, EffectStep } from "../types";
import {
  EFFECTS_CATALOG,
  formatEffectStep,
  formatRecipeSteps,
  getEffectMeta,
  isAudioCompatibleRecipe,
} from "../effectsCatalog";

const e = useEditor();

// Single source of truth: `blyacut_core::effects::builtin_recipes` (Rust).
const builtinRecipes = ref<EffectRecipe[]>([]);

// Загрузка пресетов из Rust Tauri (когда команда готова)
onMounted(async () => {
  if (!isTauri()) return;
  try {
    const list = await invoke<EffectRecipe[]>("builtin_effect_recipes");
    if (Array.isArray(list) && list.length > 0) {
      builtinRecipes.value = list;
    }
  } catch (error) {
    e.error = `Не удалось загрузить встроенные эффекты: ${error}`;
  }
});

// --- ВКЛАДКИ ГАЛЕРЕИ ПРЕСЕТОВ ---
const activeTab = ref<"video" | "audio">("video");
const searchQuery = ref("");

// Если выделен клип на аудиодорожке, автоматически переключаем на аудио-эффекты
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

// --- СТАТУС ВЫДЕЛЕНИЯ И ЛИМИТЫ ---
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
  if (!currentClip.value) return "Выберите клип на таймлайне";
  if (currentEffects.value.length >= 8)
    return "Достигнут лимит: максимум 8 рецептов на клип";
  if (totalSteps.value + recipe.steps.length > 32)
    return "Достигнут лимит: максимум 32 шага на клип";
  if (isAudioTrack.value && !isAudioCompatibleRecipe(recipe)) {
    return "Видеоэффекты нельзя применять к аудиодорожке";
  }
  return null;
}

// --- УПРАВЛЕНИЕ ЦЕПОЧКОЙ ЭФФЕКТОВ КЛИПА ---
function applyRecipe(recipe: EffectRecipe) {
  const c = currentClip.value;
  if (!c || getRecipeIncompatibilityReason(recipe)) return;

  void e.edit({
    operation: "set_effects",
    clip_id: c.id,
    effects: [...c.effects, recipe],
  });
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

// --- СОЗДАНИЕ СВОЕГО ЭФФЕКТА (ОДИНОЧНЫЙ ШАГ СО СЛАЙДЕРАМИ) ---
const customStepEffect = ref<EffectStep["effect"]>("contrast");
const customParamValues = ref<Record<string, number>>({});
const customRecipeName = ref("");

// Обновление параметров по умолчанию при смене типа шага
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

// --- ЗАГРУЗКА ПАПКИ С ДИСКА (LOAD RECIPE FOLDER) ---
const folder = ref("");
const folderCatalog = ref<EffectCatalog>({ recipes: [], warnings: [] });

async function chooseFolder() {
  const path = await open({
    directory: true,
    multiple: false,
    title: "Выберите папку с JSON-рецептами эффектов",
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
    <!-- Заголовок панели -->
    <div class="panel-header-row">
      <div class="header-left">
        <h4 class="title">
          Эффекты
          <span class="badge">{{ currentEffects.length }}</span>
        </h4>
        <span v-if="currentClip" class="clip-sub">
          Клип на {{ isAudioTrack ? "аудиодорожке" : "видеодорожке" }}
        </span>
      </div>
      <div
        v-if="currentClip"
        class="steps-count"
        :class="{ warn: totalSteps >= 28 }"
      >
        {{ currentEffects.length }}/8 рец. · {{ totalSteps }}/32 шагов
      </div>
    </div>

    <!-- 1. ЦЕПОЧКА ЭФФЕКТОВ ТЕКУЩЕГО КЛИПА -->
    <div class="section-block">
      <div class="section-title">Цепочка на клипе</div>

      <div v-if="!currentClip" class="empty-hint">
        <span
          >👆 Выберите клип на таймлайне, чтобы применить или настроить
          эффекты.</span
        >
      </div>

      <div v-else-if="!currentEffects.length" class="empty-hint">
        <span
          >На этом клипе пока нет эффектов. Выберите пресет ниже или создайте
          свой.</span
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
              title="Переместить выше"
              :disabled="index === 0"
              @click="moveRecipe(index, -1)"
            >
              ↑
            </button>
            <button
              class="icon-btn"
              title="Переместить ниже"
              :disabled="index === currentEffects.length - 1"
              @click="moveRecipe(index, 1)"
            >
              ↓
            </button>
            <button
              class="icon-btn danger"
              title="Удалить эффект"
              @click="removeRecipe(index)"
            >
              ×
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- 2. ГАЛЕРЕЯ ВСТРОЕННЫХ ПРЕСЕТОВ -->
    <div class="section-block">
      <div class="section-title-row">
        <div class="section-title">Галерея пресетов</div>
        <div class="tabs-group">
          <button
            class="tab-btn"
            :class="{ active: activeTab === 'video' }"
            :disabled="isAudioTrack"
            title="Видео-эффекты"
            @click="activeTab = 'video'"
          >
            Видео
          </button>
          <button
            class="tab-btn"
            :class="{ active: activeTab === 'audio' }"
            title="Аудио-эффекты"
            @click="activeTab = 'audio'"
          >
            Звук
          </button>
        </div>
      </div>

      <!-- Поиск по названию -->
      <div class="search-box">
        <input
          v-model="searchQuery"
          type="text"
          placeholder="Поиск пресетов (название, шаг)..."
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

      <!-- Сетка карточек пресетов (2 колонки) -->
      <div class="presets-grid">
        <div
          v-for="recipe in filteredRecipes"
          :key="recipe.id"
          class="preset-card"
          :class="{
            disabled: !currentClip || !!getRecipeIncompatibilityReason(recipe),
          }"
          :title="
            getRecipeIncompatibilityReason(recipe) ??
            'Нажмите, чтобы добавить эффект'
          "
          tabindex="0"
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
        Ничего не найдено по запросу «{{ searchQuery }}».
      </p>
    </div>

    <!-- 3. СВОЙ ЭФФЕКТ (КОНСТРУКТОР ОДИНОЧНОГО ШАГА) -->
    <details class="section-block custom-effect-details">
      <summary class="section-title summary-toggle">
        <span>Создать свой эффект</span>
      </summary>

      <div class="custom-builder-body">
        <div class="field-row">
          <label class="field-label">Тип шага</label>
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
                meta.kind === "video" ? "Видео" : "Аудио"
              }})
            </option>
          </select>
        </div>

        <div v-if="selectedStepMeta" class="step-desc-hint">
          {{ selectedStepMeta.description }}
        </div>

        <!-- Слайдеры параметров выбранного шага -->
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
          <label class="field-label">Название эффекта</label>
          <input
            v-model="customRecipeName"
            type="text"
            placeholder="Название пресета"
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
          ＋ Добавить эффект к клипу
        </button>
      </div>
    </details>

    <!-- 4. ЗАГРУЗКА ПАПКИ С ДИСКА (LOAD RECIPE FOLDER) -->
    <details class="section-block folder-details">
      <summary class="section-title summary-toggle">
        <span>Пользовательская папка с JSON-рецептами</span>
      </summary>

      <div class="folder-body">
        <button class="full" @click="chooseFolder">
          {{ folder ? "Сменить папку рецептов…" : "Load recipe folder…" }}
        </button>

        <template v-if="folder">
          <div class="folder-path-label">{{ folder }}</div>
          <button class="full secondary" @click="reloadFolder">
            Обновить рецепты из папки
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

    <!-- Честная подпись о возможностях превью и экспорта -->
    <div class="footer-disclaimer">
      💡 Экспорт применяет все эффекты. Превью — приблизительное (только
      цвет/размытие/ч-б/сепия/негатив/зеркало).
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

/* Список цепочки на клипе */
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

/* Поиск */
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

/* Сетка пресетов 2 колонки */
.presets-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
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

/* Конструктор эффекта */
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

/* Папка с диска */
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
