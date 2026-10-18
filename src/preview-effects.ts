import type { Clip, EffectRecipe, EffectStep } from "./types";
export function videoEffectStyle(recipes: EffectRecipe[]): string {
  return recipes
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
        default:
          return [];
      }
    })
    .join(" ");
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
      .filter((s) => ["low_pass", "high_pass", "echo"].includes(s.effect));
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
