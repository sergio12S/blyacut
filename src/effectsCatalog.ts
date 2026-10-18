import type { EffectRecipe, EffectStep } from "./types";
import { isVideoStep } from "./types";

export interface EffectParamDef {
  key: string;
  name: string;
  min: number;
  max: number;
  default: number;
  step: number;
  unit: string;
  format?: (value: number) => string;
}

export interface EffectMeta {
  effect: EffectStep["effect"];
  kind: "video" | "audio";
  name: string;
  icon: string;
  category: string;
  description: string;
  params: EffectParamDef[];
  createDefaultStep: () => EffectStep;
}

export const EFFECTS_CATALOG: EffectMeta[] = [
  // --- ВИДЕО: ЦВЕТ И ТОН ---
  {
    effect: "brightness",
    kind: "video",
    name: "Яркость",
    icon: "☀️",
    category: "Цвет и тон",
    description: "Увеличение или уменьшение общей яркости кадра",
    params: [
      {
        key: "gain_milli",
        name: "Уровень",
        min: 0,
        max: 2000,
        default: 1000,
        step: 50,
        unit: "x",
        format: (v) => `${(v / 1000).toFixed(2)}x`,
      },
    ],
    createDefaultStep: () => ({ effect: "brightness", gain_milli: 1200 }),
  },
  {
    effect: "contrast",
    kind: "video",
    name: "Контраст",
    icon: "🌓",
    category: "Цвет и тон",
    description: "Контрастность между светлыми и темными областями",
    params: [
      {
        key: "gain_milli",
        name: "Уровень",
        min: 0,
        max: 3000,
        default: 1000,
        step: 50,
        unit: "x",
        format: (v) => `${(v / 1000).toFixed(2)}x`,
      },
    ],
    createDefaultStep: () => ({ effect: "contrast", gain_milli: 1250 }),
  },
  {
    effect: "saturation",
    kind: "video",
    name: "Насыщенность",
    icon: "🎨",
    category: "Цвет и тон",
    description: "Интенсивность и сочность цветов",
    params: [
      {
        key: "gain_milli",
        name: "Уровень",
        min: 0,
        max: 3000,
        default: 1000,
        step: 50,
        unit: "x",
        format: (v) => `${(v / 1000).toFixed(2)}x`,
      },
    ],
    createDefaultStep: () => ({ effect: "saturation", gain_milli: 1300 }),
  },
  {
    effect: "hue",
    kind: "video",
    name: "Цветовой тон",
    icon: "🌈",
    category: "Цвет и тон",
    description: "Сдвиг цветового круга (Hue rotation)",
    params: [
      {
        key: "degrees",
        name: "Угол",
        min: -360,
        max: 360,
        default: 0,
        step: 5,
        unit: "°",
        format: (v) => `${v}°`,
      },
    ],
    createDefaultStep: () => ({ effect: "hue", degrees: 45 }),
  },
  {
    effect: "gamma",
    kind: "video",
    name: "Гамма",
    icon: "⚡",
    category: "Цвет и тон",
    description: "Нелинейная коррекция полутонов",
    params: [
      {
        key: "gamma_milli",
        name: "Гамма",
        min: 100,
        max: 3000,
        default: 1000,
        step: 50,
        unit: "x",
        format: (v) => `${(v / 1000).toFixed(2)}x`,
      },
    ],
    createDefaultStep: () => ({ effect: "gamma", gamma_milli: 1200 }),
  },
  {
    effect: "vibrance",
    kind: "video",
    name: "Красочность (Vibrance)",
    icon: "✨",
    category: "Цвет и тон",
    description: "Усиление ненасыщенных цветов без перенасыщения кожи",
    params: [
      {
        key: "intensity_milli",
        name: "Интенсивность",
        min: -2000,
        max: 2000,
        default: 0,
        step: 50,
        unit: "m",
        format: (v) => `${(v / 1000).toFixed(2)}`,
      },
    ],
    createDefaultStep: () => ({ effect: "vibrance", intensity_milli: 500 }),
  },
  {
    effect: "temperature",
    kind: "video",
    name: "Цветовая температура",
    icon: "🌡️",
    category: "Цвет и тон",
    description: "Баланс белого: холодный (<6500K) или теплый (>6500K)",
    params: [
      {
        key: "kelvin",
        name: "Температура",
        min: 1000,
        max: 40000,
        default: 6500,
        step: 250,
        unit: "K",
        format: (v) => `${v} K`,
      },
    ],
    createDefaultStep: () => ({ effect: "temperature", kelvin: 7500 }),
  },
  {
    effect: "grayscale",
    kind: "video",
    name: "Черно-белое",
    icon: "🔘",
    category: "Цвет и тон",
    description: "Преобразование видео в монохромное изображение",
    params: [],
    createDefaultStep: () => ({ effect: "grayscale" }),
  },
  {
    effect: "sepia",
    kind: "video",
    name: "Сепия",
    icon: "📜",
    category: "Цвет и тон",
    description: "Теплый винтажный тон под старинную фотографию",
    params: [],
    createDefaultStep: () => ({ effect: "sepia" }),
  },
  {
    effect: "invert",
    kind: "video",
    name: "Инверсия цветов",
    icon: "🔄",
    category: "Цвет и тон",
    description: "Негатив: обращение всех цветов кадра",
    params: [],
    createDefaultStep: () => ({ effect: "invert" }),
  },

  // --- ВИДЕО: РАЗМЫТИЕ И ЧЕТКОСТЬ ---
  {
    effect: "blur",
    kind: "video",
    name: "Размытие (Blur)",
    icon: "💧",
    category: "Размытие и четкость",
    description: "Мягкое размытие по Гауссу / боксу",
    params: [
      {
        key: "strength",
        name: "Сила",
        min: 1,
        max: 50,
        default: 5,
        step: 1,
        unit: "px",
        format: (v) => `${v} px`,
      },
    ],
    createDefaultStep: () => ({ effect: "blur", strength: 8 }),
  },
  {
    effect: "sharpen",
    kind: "video",
    name: "Резкость (Sharpen)",
    icon: "🗡️",
    category: "Размытие и четкость",
    description: "Повышение четкости и контуров деталей",
    params: [
      {
        key: "amount_milli",
        name: "Сила",
        min: 0,
        max: 3000,
        default: 1000,
        step: 50,
        unit: "m",
        format: (v) => `${(v / 1000).toFixed(2)}x`,
      },
    ],
    createDefaultStep: () => ({ effect: "sharpen", amount_milli: 1200 }),
  },

  // --- ВИДЕО: СТИЛИЗАЦИЯ И ГЛИТЧ ---
  {
    effect: "vignette",
    kind: "video",
    name: "Виньетка",
    icon: "🔲",
    category: "Стилизация и глитч",
    description: "Затемнение краев кадра для кино-акцента на центре",
    params: [
      {
        key: "strength_milli",
        name: "Плотность",
        min: 0,
        max: 1000,
        default: 300,
        step: 25,
        unit: "m",
        format: (v) => `${Math.round(v / 10)}%`,
      },
    ],
    createDefaultStep: () => ({ effect: "vignette", strength_milli: 400 }),
  },
  {
    effect: "film_grain",
    kind: "video",
    name: "Зерно пленки",
    icon: "🎞️",
    category: "Стилизация и глитч",
    description: "Аналоговый пленочный шум и текстура",
    params: [
      {
        key: "strength",
        name: "Интенсивность",
        min: 1,
        max: 100,
        default: 15,
        step: 1,
        unit: "%",
        format: (v) => `${v}%`,
      },
    ],
    createDefaultStep: () => ({ effect: "film_grain", strength: 20 }),
  },
  {
    effect: "pixelate",
    kind: "video",
    name: "Пикселизация",
    icon: "👾",
    category: "Стилизация и глитч",
    description: "Ретро 8-битная мозаика крупными блоками",
    params: [
      {
        key: "block",
        name: "Размер блока",
        min: 2,
        max: 64,
        default: 8,
        step: 1,
        unit: "px",
        format: (v) => `${v} px`,
      },
    ],
    createDefaultStep: () => ({ effect: "pixelate", block: 8 }),
  },
  {
    effect: "posterize",
    kind: "video",
    name: "Постеризация",
    icon: "🖼️",
    category: "Стилизация и глитч",
    description: "Уменьшение количества градаций цвета под плакат",
    params: [
      {
        key: "levels",
        name: "Уровни",
        min: 2,
        max: 32,
        default: 6,
        step: 1,
        unit: "lvl",
        format: (v) => `${v}`,
      },
    ],
    createDefaultStep: () => ({ effect: "posterize", levels: 6 }),
  },
  {
    effect: "chromatic_aberration",
    kind: "video",
    name: "Хроматическая аберрация",
    icon: "👓",
    category: "Стилизация и глитч",
    description: "RGB-расщепление каналов (глитч-эффект линзы)",
    params: [
      {
        key: "pixels",
        name: "Сдвиг",
        min: 1,
        max: 40,
        default: 4,
        step: 1,
        unit: "px",
        format: (v) => `${v} px`,
      },
    ],
    createDefaultStep: () => ({ effect: "chromatic_aberration", pixels: 5 }),
  },
  {
    effect: "edge_detect",
    kind: "video",
    name: "Выделение краев",
    icon: "📐",
    category: "Стилизация и глитч",
    description: "Неоновый контурный фильтр границ объектов",
    params: [],
    createDefaultStep: () => ({ effect: "edge_detect" }),
  },

  // --- ВИДЕО: ТРАНСФОРМАЦИЯ И ПЕРЕХОДЫ ---
  {
    effect: "mirror",
    kind: "video",
    name: "Отразить по горизонтали",
    icon: "🪞",
    category: "Трансформация",
    description: "Зеркальный поворот кадра влево-вправо",
    params: [],
    createDefaultStep: () => ({ effect: "mirror" }),
  },
  {
    effect: "flip",
    kind: "video",
    name: "Отразить по вертикали",
    icon: "🙃",
    category: "Трансформация",
    description: "Переворот кадра вверх ногами",
    params: [],
    createDefaultStep: () => ({ effect: "flip" }),
  },
  {
    effect: "ghost_trails",
    kind: "video",
    name: "Шлейф движения (Ghost Trails)",
    icon: "👻",
    category: "Трансформация",
    description: "Остаточный шлейф кадров позади движущихся объектов",
    params: [
      {
        key: "decay_milli",
        name: "Затухание",
        min: 0,
        max: 990,
        default: 500,
        step: 25,
        unit: "m",
        format: (v) => `${Math.round(v / 10)}%`,
      },
    ],
    createDefaultStep: () => ({ effect: "ghost_trails", decay_milli: 600 }),
  },
  {
    effect: "fade_from_black",
    kind: "video",
    name: "Появление из черного",
    icon: "🎬",
    category: "Переходы",
    description: "Плавное появление видео в начале клипа",
    params: [
      {
        key: "duration_ms",
        name: "Длительность",
        min: 1,
        max: 60000,
        default: 1000,
        step: 100,
        unit: "мс",
        format: (v) => `${(v / 1000).toFixed(1)} с`,
      },
    ],
    createDefaultStep: () => ({ effect: "fade_from_black", duration_ms: 1000 }),
  },
  {
    effect: "fade_to_black",
    kind: "video",
    name: "Уход в черное",
    icon: "🌑",
    category: "Переходы",
    description: "Плавное затухание видео в конце клипа",
    params: [
      {
        key: "duration_ms",
        name: "Длительность",
        min: 1,
        max: 60000,
        default: 1000,
        step: 100,
        unit: "мс",
        format: (v) => `${(v / 1000).toFixed(1)} с`,
      },
    ],
    createDefaultStep: () => ({ effect: "fade_to_black", duration_ms: 1000 }),
  },

  // --- АУДИО: ЧАСТОТЫ И ЭКВАЛАЙЗЕР ---
  {
    effect: "low_pass",
    kind: "audio",
    name: "Срез высоких (Low-Pass)",
    icon: "🔉",
    category: "Частоты и эквалайзер",
    description: "Срезает высокие частоты, создавая эффект 'звук за дверью'",
    params: [
      {
        key: "frequency_hz",
        name: "Частота среза",
        min: 20,
        max: 20000,
        default: 1200,
        step: 50,
        unit: "Гц",
        format: (v) => (v >= 1000 ? `${(v / 1000).toFixed(1)} кГц` : `${v} Гц`),
      },
    ],
    createDefaultStep: () => ({ effect: "low_pass", frequency_hz: 1200 }),
  },
  {
    effect: "high_pass",
    kind: "audio",
    name: "Срез низких (High-Pass)",
    icon: "📻",
    category: "Частоты и эквалайзер",
    description: "Убирает гул и бубнеж, оставляя чистый голос",
    params: [
      {
        key: "frequency_hz",
        name: "Частота среза",
        min: 20,
        max: 20000,
        default: 100,
        step: 10,
        unit: "Гц",
        format: (v) => (v >= 1000 ? `${(v / 1000).toFixed(1)} кГц` : `${v} Гц`),
      },
    ],
    createDefaultStep: () => ({ effect: "high_pass", frequency_hz: 85 }),
  },
  {
    effect: "bass",
    kind: "audio",
    name: "Низкие частоты (Bass)",
    icon: "🔊",
    category: "Частоты и эквалайзер",
    description: "Усиление или ослабление баса (100 Гц)",
    params: [
      {
        key: "gain_db",
        name: "Усиление",
        min: -20,
        max: 20,
        default: 0,
        step: 0.5,
        unit: "дБ",
        format: (v) => `${v > 0 ? "+" : ""}${v} дБ`,
      },
    ],
    createDefaultStep: () => ({ effect: "bass", gain_db: 4 }),
  },
  {
    effect: "treble",
    kind: "audio",
    name: "Высокие частоты (Treble)",
    icon: "🔔",
    category: "Частоты и эквалайзер",
    description: "Усиление или ослабление прозрачности верхов (10 кГц)",
    params: [
      {
        key: "gain_db",
        name: "Усиление",
        min: -20,
        max: 20,
        default: 0,
        step: 0.5,
        unit: "дБ",
        format: (v) => `${v > 0 ? "+" : ""}${v} дБ`,
      },
    ],
    createDefaultStep: () => ({ effect: "treble", gain_db: 3 }),
  },
  {
    effect: "equalizer",
    kind: "audio",
    name: "Параметрический эквалайзер",
    icon: "🎚️",
    category: "Частоты и эквалайзер",
    description: "Точная регулировка усиления на заданной частоте",
    params: [
      {
        key: "frequency_hz",
        name: "Частота",
        min: 20,
        max: 20000,
        default: 1000,
        step: 50,
        unit: "Гц",
        format: (v) => (v >= 1000 ? `${(v / 1000).toFixed(1)} кГц` : `${v} Гц`),
      },
      {
        key: "gain_db",
        name: "Усиление",
        min: -20,
        max: 20,
        default: 0,
        step: 0.5,
        unit: "дБ",
        format: (v) => `${v > 0 ? "+" : ""}${v} дБ`,
      },
    ],
    createDefaultStep: () => ({
      effect: "equalizer",
      frequency_hz: 3000,
      gain_db: 3,
    }),
  },

  // --- АУДИО: ПРОСТРАНСТВО И ДИНАМИКА ---
  {
    effect: "echo",
    kind: "audio",
    name: "Эхо / Дилэй",
    icon: "⛰️",
    category: "Пространство и задержка",
    description: "Ритмические повторы с затуханием",
    params: [
      {
        key: "delay_ms",
        name: "Задержка",
        min: 1,
        max: 2000,
        default: 250,
        step: 10,
        unit: "мс",
        format: (v) => `${v} мс`,
      },
      {
        key: "decay_milli",
        name: "Затухание",
        min: 0,
        max: 900,
        default: 400,
        step: 25,
        unit: "m",
        format: (v) => `${Math.round(v / 10)}%`,
      },
    ],
    createDefaultStep: () => ({
      effect: "echo",
      delay_ms: 240,
      decay_milli: 350,
    }),
  },
  {
    effect: "reverb",
    kind: "audio",
    name: "Реверберация (Reverb)",
    icon: "🏛️",
    category: "Пространство и задержка",
    description: "Акустика зала или комнаты",
    params: [
      {
        key: "room_milli",
        name: "Размер комнаты",
        min: 0,
        max: 1000,
        default: 500,
        step: 25,
        unit: "m",
        format: (v) => `${Math.round(v / 10)}%`,
      },
    ],
    createDefaultStep: () => ({ effect: "reverb", room_milli: 450 }),
  },
  {
    effect: "auto_pan",
    kind: "audio",
    name: "Авто-панорама",
    icon: "↔️",
    category: "Пространство и задержка",
    description: "Плавное перетекание звука между левым и правым каналами",
    params: [
      {
        key: "frequency_milli",
        name: "Скорость",
        min: 10,
        max: 20000,
        default: 500,
        step: 50,
        unit: "mHz",
        format: (v) => `${(v / 1000).toFixed(2)} Гц`,
      },
    ],
    createDefaultStep: () => ({ effect: "auto_pan", frequency_milli: 600 }),
  },
  {
    effect: "stereo_widen",
    kind: "audio",
    name: "Расширение стерео",
    icon: "🎧",
    category: "Пространство и задержка",
    description: "Увеличение стереобазы и объема звучания",
    params: [
      {
        key: "amount_milli",
        name: "Ширина",
        min: 0,
        max: 10000,
        default: 1000,
        step: 100,
        unit: "m",
        format: (v) => `${(v / 1000).toFixed(1)}x`,
      },
    ],
    createDefaultStep: () => ({ effect: "stereo_widen", amount_milli: 1500 }),
  },
  {
    effect: "compressor",
    kind: "audio",
    name: "Компрессор",
    icon: "🗜️",
    category: "Динамика и уровень",
    description:
      "Выравнивание динамического диапазона (делает шепот громче, пики тише)",
    params: [
      {
        key: "threshold_db",
        name: "Порог (Threshold)",
        min: -60,
        max: 0,
        default: -18,
        step: 1,
        unit: "дБ",
        format: (v) => `${v} дБ`,
      },
      {
        key: "ratio",
        name: "Степень (Ratio)",
        min: 1,
        max: 20,
        default: 4,
        step: 0.5,
        unit: ":1",
        format: (v) => `${v}:1`,
      },
    ],
    createDefaultStep: () => ({
      effect: "compressor",
      threshold_db: -20,
      ratio: 4,
    }),
  },
  {
    effect: "denoise",
    kind: "audio",
    name: "Шумоподавление",
    icon: "🧹",
    category: "Динамика и уровень",
    description: "Подавление фонового статического шума микрофона",
    params: [
      {
        key: "strength_db",
        name: "Сила подавления",
        min: 1,
        max: 90,
        default: 20,
        step: 1,
        unit: "дБ",
        format: (v) => `${v} дБ`,
      },
    ],
    createDefaultStep: () => ({ effect: "denoise", strength_db: 24 }),
  },
  {
    effect: "normalize",
    kind: "audio",
    name: "Нормализация",
    icon: "📊",
    category: "Динамика и уровень",
    description: "Приведение пиков к стандартному уровню без искажений",
    params: [],
    createDefaultStep: () => ({ effect: "normalize" }),
  },

  // --- АУДИО: МОДУЛЯЦИЯ И ТВОРЧЕСТВО ---
  {
    effect: "pitch",
    kind: "audio",
    name: "Высота тона (Pitch)",
    icon: "🎵",
    category: "Модуляция и эффекты",
    description: "Сдвиг тональности голоса вверх или вниз",
    params: [
      {
        key: "semitones",
        name: "Полутоны",
        min: -12,
        max: 12,
        default: 0,
        step: 1,
        unit: "st",
        format: (v) => `${v > 0 ? "+" : ""}${v} п/т`,
      },
    ],
    createDefaultStep: () => ({ effect: "pitch", semitones: 2 }),
  },
  {
    effect: "tremolo",
    kind: "audio",
    name: "Тремоло (Tremolo)",
    icon: "〰️",
    category: "Модуляция и эффекты",
    description: "Периодическая пульсация громкости звука",
    params: [
      {
        key: "frequency_milli",
        name: "Частота",
        min: 100,
        max: 20000,
        default: 4000,
        step: 100,
        unit: "mHz",
        format: (v) => `${(v / 1000).toFixed(1)} Гц`,
      },
      {
        key: "depth_milli",
        name: "Глубина",
        min: 0,
        max: 1000,
        default: 500,
        step: 25,
        unit: "m",
        format: (v) => `${Math.round(v / 10)}%`,
      },
    ],
    createDefaultStep: () => ({
      effect: "tremolo",
      frequency_milli: 4000,
      depth_milli: 600,
    }),
  },
  {
    effect: "vibrato",
    kind: "audio",
    name: "Вибрато (Vibrato)",
    icon: "📯",
    category: "Модуляция и эффекты",
    description: "Периодическая пульсация высоты тона",
    params: [
      {
        key: "frequency_milli",
        name: "Частота",
        min: 100,
        max: 20000,
        default: 5000,
        step: 100,
        unit: "mHz",
        format: (v) => `${(v / 1000).toFixed(1)} Гц`,
      },
      {
        key: "depth_milli",
        name: "Глубина",
        min: 0,
        max: 1000,
        default: 400,
        step: 25,
        unit: "m",
        format: (v) => `${Math.round(v / 10)}%`,
      },
    ],
    createDefaultStep: () => ({
      effect: "vibrato",
      frequency_milli: 5000,
      depth_milli: 400,
    }),
  },
  {
    effect: "bitcrush",
    kind: "audio",
    name: "Биткрашер (Bitcrush)",
    icon: "🕹️",
    category: "Модуляция и эффекты",
    description: "Уменьшение разрядности для звука старых игровых консолей",
    params: [
      {
        key: "bits",
        name: "Разрядность",
        min: 1,
        max: 16,
        default: 8,
        step: 1,
        unit: "бит",
        format: (v) => `${v} бит`,
      },
    ],
    createDefaultStep: () => ({ effect: "bitcrush", bits: 8 }),
  },
  {
    effect: "chorus",
    kind: "audio",
    name: "Хорус (Chorus)",
    icon: "👥",
    category: "Модуляция и эффекты",
    description: "Эффект ансамбля или хорового звучания нескольких голосов",
    params: [],
    createDefaultStep: () => ({ effect: "chorus" }),
  },
  {
    effect: "flanger",
    kind: "audio",
    name: "Флэнжер (Flanger)",
    icon: "✈️",
    category: "Модуляция и эффекты",
    description: "Космический взлетающий реактивный свист",
    params: [],
    createDefaultStep: () => ({ effect: "flanger" }),
  },
  {
    effect: "phaser",
    kind: "audio",
    name: "Фейзер (Phaser)",
    icon: "🌀",
    category: "Модуляция и эффекты",
    description: "Фазовые пульсации и космическое закручивание",
    params: [],
    createDefaultStep: () => ({ effect: "phaser" }),
  },
  {
    effect: "robot",
    kind: "audio",
    name: "Голос робота",
    icon: "🤖",
    category: "Модуляция и эффекты",
    description: "Металлический вокодерный синтетический тембр",
    params: [],
    createDefaultStep: () => ({ effect: "robot" }),
  },
];

const CATALOG_BY_EFFECT = new Map<EffectStep["effect"], EffectMeta>(
  EFFECTS_CATALOG.map((item) => [item.effect, item]),
);

export function getEffectMeta(
  effectName: EffectStep["effect"],
): EffectMeta | undefined {
  return CATALOG_BY_EFFECT.get(effectName);
}

/** Formats a single EffectStep into a readable Russian summary */
export function formatEffectStep(step: EffectStep): string {
  const meta = getEffectMeta(step.effect);
  if (!meta) return step.effect;

  if (meta.params.length === 0) {
    return `${meta.icon} ${meta.name}`;
  }

  const parts: string[] = [];
  for (const p of meta.params) {
    const rawVal = (step as Record<string, unknown>)[p.key];
    if (typeof rawVal === "number") {
      parts.push(p.format ? p.format(rawVal) : `${rawVal}${p.unit}`);
    }
  }

  return `${meta.icon} ${meta.name}${parts.length ? ` (${parts.join(", ")})` : ""}`;
}

/** Formats an entire EffectRecipe into a sequence of steps */
export function formatRecipeSteps(recipe: EffectRecipe): string {
  return recipe.steps
    .map((s) => {
      const meta = getEffectMeta(s.effect);
      return meta ? `${meta.icon} ${meta.name}` : s.effect;
    })
    .join(" → ");
}

/** Returns true if all steps in the recipe are compatible with audio tracks */
export function isAudioCompatibleRecipe(recipe: EffectRecipe): boolean {
  return recipe.steps.every((step) => !isVideoStep(step));
}
