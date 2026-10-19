import type { TransitionKind } from "./types";
/** Russian names and pictograms for the transition library; kinds mirror Rust `TransitionKind`. */
export const TRANSITION_NAMES: Record<TransitionKind, string> = {
  fade: "Наплыв",
  fadeblack: "Через чёрное",
  fadewhite: "Через белое",
  dissolve: "Растворение",
  wipeleft: "Шторка влево",
  wiperight: "Шторка вправо",
  wipeup: "Шторка вверх",
  wipedown: "Шторка вниз",
  slideleft: "Сдвиг влево",
  slideright: "Сдвиг вправо",
  slideup: "Сдвиг вверх",
  slidedown: "Сдвиг вниз",
  smoothleft: "Мягко влево",
  smoothright: "Мягко вправо",
  circleopen: "Круг из центра",
  circleclose: "Круг к центру",
  radial: "Радар",
  pixelize: "Пиксели",
  zoomin: "Наезд",
  hblur: "Размытие",
};
export const TRANSITION_ICONS: Record<TransitionKind, string> = {
  fade: "◐",
  fadeblack: "◑",
  fadewhite: "○",
  dissolve: "░",
  wipeleft: "⇤",
  wiperight: "⇥",
  wipeup: "⤒",
  wipedown: "⤓",
  slideleft: "◀",
  slideright: "▶",
  slideup: "▲",
  slidedown: "▼",
  smoothleft: "↜",
  smoothright: "↝",
  circleopen: "◎",
  circleclose: "◉",
  radial: "◴",
  pixelize: "▦",
  zoomin: "⊕",
  hblur: "≋",
};
export const TRANSITION_KINDS = Object.keys(
  TRANSITION_NAMES,
) as TransitionKind[];
/** Drag payload: a transition kind dropped onto a main-track clip. */
export const TRANSITION_MIME = "application/x-blyacut-transition";
