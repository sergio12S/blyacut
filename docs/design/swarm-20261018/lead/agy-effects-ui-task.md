# Задача для agy: UI «куча эффектов» (лид: claude-ux-20261018)

Ветка: `agent/effects-pack` (уже активна в <repo>). НЕ коммить, НЕ переключай ветки — коммитит лид.

## Твои файлы (только их)
- `src/components/EffectsPanel.vue` — переписать.
- `src/effectsCatalog.ts` — новый: русские названия, иконки (эмодзи/символ), категории, описание параметров для каждого `EffectStep["effect"]`.
- `src/components/Preview.vue` — ТОЛЬКО добавить приблизительное CSS-превью видео-эффектов выделенного/активного клипа.

НЕ трогай: `crates/`, `src-tauri/`, `src/types.ts`, `src/stores/`, `src/App.vue`, `src/style.css`. Стили — `<style scoped>` внутри своих .vue. Нужна правка в чужом файле — напиши лиду.

## Контракт (уже есть)
- `src/types.ts`: расширенный `EffectStep` (23 видео + 18 аудио), `VIDEO_EFFECTS`, `isVideoStep`.
- Новая Tauri-команда (лид делает в Rust): `invoke<EffectRecipe[]>("builtin_effect_recipes")` — встроенные пресеты (id начинается с `builtin-`), ~30 штук, видео и аудио. Загрузка папки `load_effect_recipes` остаётся.
- Применение: `e.edit({operation:"set_effects", clip_id, effects:[...clip.effects, recipe]})`. Лимит: ≤ 8 рецептов на клип, ≤ 32 шага суммарно. На аудиодорожке видео-рецепты запрещены (кнопка disabled + подсказка).

## EffectsPanel.vue — что сделать
1. Галерея встроенных пресетов, всегда видна (без выбора папки): вкладки «Видео / Звук», поиск по названию, сетка карточек 2 колонки (имя + цепочка шагов по-русски из effectsCatalog). Клик/Enter = добавить к выделенному клипу. Без выделения — карточки disabled, подсказка «Выберите клип».
2. Цепочка эффектов клипа: список рецептов, кнопки ↑ ↓ (перестановка) и × (удалить) — каждое действие = один `set_effects`.
3. «Свой эффект»: выпадающий список одиночных шагов из effectsCatalog + слайдеры параметров с диапазонами (как в Rust, см. ниже) → кнопка «Добавить» создаёт рецепт `{version:1,id:"custom-"+Date.now(), name, steps:[step]}`.
4. Оставить «Load recipe folder…» внизу, свёрнутым.
5. Подпись честно: «Экспорт применяет все эффекты. Превью — приблизительное (только цвет/размытие/ч-б/сепия/негатив/зеркало)».

## Диапазоны параметров (Rust валидирует так же)
brightness/contrast/saturation gain_milli 0..2000/3000/3000 (1000 = без изменений); hue degrees −360..360; gamma gamma_milli 100..3000; vibrance intensity_milli −2000..2000; temperature kelvin 1000..40000 (6500 нейтр.); blur strength 1..50; sharpen amount_milli 0..3000; vignette strength_milli 0..1000; film_grain strength 1..100; pixelate block 2..64; posterize levels 2..32; chromatic_aberration pixels 1..40; ghost_trails decay_milli 0..990; fade_from_black/fade_to_black duration_ms 1..60000;
low_pass/high_pass frequency_hz 20..20000; echo delay_ms 1..2000 decay_milli 0..900; bass/treble gain_db −20..20; equalizer frequency_hz 20..20000 gain_db −20..20; reverb room_milli 0..1000; compressor threshold_db −60..0 ratio 1..20; pitch semitones −12..12; tremolo/vibrato frequency_milli 100..20000 depth_milli 0..1000; auto_pan frequency_milli 10..20000; bitcrush bits 1..16; denoise strength_db 1..90; stereo_widen amount_milli 0..10000. Без параметров: grayscale sepia invert mirror flip edge_detect chorus flanger phaser robot normalize.

## Preview.vue — CSS-превью
Для клипа под плейхедом на видеодорожке собрать `style.filter` у `<video>`: brightness(g)/contrast(g)/saturate(g)/hue-rotate(deg)/blur(px≈strength/2)/grayscale(1)/sepia(1)/invert(1); mirror → `transform: scaleX(-1)`, flip → `scaleY(-1)`. Остальное игнорировать. Маленькая плашка «превью ≈» в углу, если у клипа есть эффекты, которых нет в CSS.

## Проверка
`npm run build` должен пройти (vue-tsc). Потом: `gbrain_signal_emit(topic="agent.signal", content={"to":"claude-ux-20261018","message":"effects UI done: ..."})` и кратко отпиши в своём терминале.
