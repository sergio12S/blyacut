import type { Clip, EffectRecipe, EffectStep } from "./types";
/** Steps the monitor can approximate; everything else is export-only. */
export const PREVIEW_VIDEO = new Set<EffectStep["effect"]>([
  "brightness",
  "contrast",
  "saturation",
  "hue",
  "blur",
  "grayscale",
  "sepia",
  "invert",
  "mirror",
  "flip",
]);
export const PREVIEW_AUDIO = new Set<EffectStep["effect"]>([
  "low_pass",
  "high_pass",
  "echo",
  "bass",
  "treble",
  "equalizer",
  "compressor",
  "tremolo",
  "auto_pan",
  "bitcrush",
]);
export const previewIsApproximate = (recipes: EffectRecipe[]) =>
  recipes
    .flatMap((r) => r.steps)
    .some((s) => !PREVIEW_VIDEO.has(s.effect) && !PREVIEW_AUDIO.has(s.effect));
export function videoEffectStyle(recipes: EffectRecipe[]): {
  filter?: string;
  transform?: string;
} {
  let x = 1,
    y = 1;
  const filter = recipes
    .flatMap((r) => r.steps)
    .flatMap((step) => {
      switch (step.effect) {
        case "brightness":
          return [`brightness(${step.gain_milli / 1000})`];
        case "contrast":
          return [`contrast(${step.gain_milli / 1000})`];
        case "saturation":
          return [`saturate(${step.gain_milli / 1000})`];
        case "hue":
          return [`hue-rotate(${step.degrees}deg)`];
        case "blur":
          return [`blur(${step.strength / 2}px)`];
        case "grayscale":
          return ["grayscale(1)"];
        case "sepia":
          return ["sepia(1)"];
        case "invert":
          return ["invert(1)"];
        case "mirror":
          x = -x;
          return [];
        case "flip":
          y = -y;
          return [];
        default:
          return [];
      }
    })
    .join(" ");
  return {
    filter: filter || undefined,
    transform: x !== 1 || y !== 1 ? `scale(${x}, ${y})` : undefined,
  };
}
interface Graph {
  source: MediaElementAudioSourceNode;
  output: GainNode;
  nodes: AudioNode[];
  key: string;
  connected: boolean;
}
/** Browser approximation of the portable audio recipe, with no dynamically executed code. */
export class AudioEffectPreview {
  private context?: AudioContext;
  private cache = new WeakMap<HTMLMediaElement, Graph>();
  private live = new Set<HTMLMediaElement>();
  constructor(private onError: (error: unknown) => void) {}
  update(
    element: HTMLMediaElement,
    clip: Clip,
    gain: number,
    playing: boolean,
    reset = false,
  ) {
    const steps = clip.effects
      .flatMap((r) => r.steps)
      .filter((s) => PREVIEW_AUDIO.has(s.effect));
    let graph = this.cache.get(element);
    if (!steps.length && !graph) {
      element.volume = gain;
      return;
    }
    if (!this.context && !playing) {
      element.volume = gain;
      return;
    }
    const context = this.context ?? (this.context = new AudioContext());
    if (!graph) {
      graph = {
        source: context.createMediaElementSource(element),
        output: context.createGain(),
        nodes: [],
        key: "",
        connected: false,
      };
      this.cache.set(element, graph);
    }
    const key = JSON.stringify(steps);
    if (reset || graph.key !== key || !graph.connected)
      this.rebuild(graph, steps, context, key);
    this.live.add(element);
    element.volume = 1;
    graph.output.gain.value = playing ? gain : 0;
    if (playing && context.state === "suspended")
      void context.resume().catch(this.onError);
  }
  private rebuild(
    graph: Graph,
    steps: EffectStep[],
    context: AudioContext,
    key: string,
  ) {
    graph.source.disconnect();
    graph.nodes.forEach((node) => node.disconnect());
    graph.output.disconnect();
    graph.nodes = [];
    let current: AudioNode = graph.source;
    for (const step of steps) {
      if (step.effect === "low_pass" || step.effect === "high_pass") {
        const filter = context.createBiquadFilter();
        filter.type = step.effect === "low_pass" ? "lowpass" : "highpass";
        filter.frequency.value = step.frequency_hz;
        filter.Q.value = Math.SQRT1_2;
        current.connect(filter);
        current = filter;
        graph.nodes.push(filter);
      } else if (step.effect === "echo") {
        const delay = context.createDelay(2);
        delay.delayTime.value = step.delay_ms / 1000;
        const wet = context.createGain();
        wet.gain.value = step.decay_milli / 1000;
        const sum = context.createGain();
        current.connect(sum);
        current.connect(delay);
        delay.connect(wet);
        wet.connect(sum);
        current = sum;
        graph.nodes.push(delay, wet, sum);
      } else if (
        step.effect === "bass" ||
        step.effect === "treble" ||
        step.effect === "equalizer"
      ) {
        const filter = context.createBiquadFilter();
        filter.type =
          step.effect === "bass"
            ? "lowshelf"
            : step.effect === "treble"
              ? "highshelf"
              : "peaking";
        filter.frequency.value =
          step.effect === "equalizer"
            ? step.frequency_hz
            : step.effect === "bass"
              ? 100
              : 3000;
        filter.gain.value = step.gain_db;
        current.connect(filter);
        current = filter;
        graph.nodes.push(filter);
      } else if (step.effect === "compressor") {
        const comp = context.createDynamicsCompressor();
        comp.threshold.value = step.threshold_db;
        comp.ratio.value = step.ratio;
        current.connect(comp);
        current = comp;
        graph.nodes.push(comp);
      } else if (step.effect === "tremolo" || step.effect === "auto_pan") {
        const lfo = context.createOscillator();
        lfo.frequency.value = step.frequency_milli / 1000;
        const depth = context.createGain();
        let node: GainNode | StereoPannerNode;
        if (step.effect === "tremolo") {
          node = context.createGain();
          const d = step.depth_milli / 1000;
          node.gain.value = 1 - d / 2;
          depth.gain.value = d / 2;
          lfo.connect(depth).connect(node.gain);
        } else {
          node = context.createStereoPanner();
          depth.gain.value = 1;
          lfo.connect(depth).connect(node.pan);
        }
        lfo.start();
        current.connect(node);
        current = node;
        graph.nodes.push(lfo, depth, node);
      } else if (step.effect === "bitcrush") {
        const shaper = context.createWaveShaper();
        const levels = 2 ** step.bits;
        const curve = new Float32Array(4096);
        for (let i = 0; i < curve.length; i++) {
          const x = (i / (curve.length - 1)) * 2 - 1;
          curve[i] = Math.round(x * levels) / levels;
        }
        shaper.curve = curve;
        current.connect(shaper);
        current = shaper;
        graph.nodes.push(shaper);
      }
    }
    current.connect(graph.output);
    graph.output.connect(context.destination);
    graph.key = key;
    graph.connected = true;
  }
  silence(element: HTMLMediaElement) {
    const graph = this.cache.get(element);
    if (graph) graph.output.gain.value = 0;
  }
  prune() {
    for (const element of this.live) {
      if (!element.isConnected) {
        const graph = this.cache.get(element);
        if (graph) {
          graph.source.disconnect();
          graph.nodes.forEach((node) => node.disconnect());
          graph.output.disconnect();
          graph.connected = false;
        }
        this.live.delete(element);
      }
    }
  }
  dispose() {
    for (const element of this.live) this.silence(element);
    this.live.clear();
    if (this.context) void this.context.close().catch(this.onError);
  }
}
