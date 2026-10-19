import type { TransitionKind } from "./types";
/** Display names and pictograms for the transition library; kinds mirror Rust `TransitionKind`. */
export const TRANSITION_NAMES: Record<TransitionKind, string> = {
  fade: "Crossfade",
  fadeblack: "Dip to black",
  fadewhite: "Dip to white",
  dissolve: "Dissolve",
  wipeleft: "Wipe left",
  wiperight: "Wipe right",
  wipeup: "Wipe up",
  wipedown: "Wipe down",
  slideleft: "Slide left",
  slideright: "Slide right",
  slideup: "Slide up",
  slidedown: "Slide down",
  smoothleft: "Smooth left",
  smoothright: "Smooth right",
  circleopen: "Circle open",
  circleclose: "Circle close",
  radial: "Radial",
  pixelize: "Pixelize",
  zoomin: "Zoom in",
  hblur: "Blur",
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
