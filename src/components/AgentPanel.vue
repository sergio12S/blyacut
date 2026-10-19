<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { describeEdit } from "../edit-description";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useEditor } from "../stores/editor";
import { clipLength } from "../timeline-gesture";
import logo from "../assets/logo-mark.svg";
import {
  time,
  type AgentConnection,
  type Proposal,
  type Snapshot,
} from "../types";
const e = useEditor();
const connection = ref<AgentConnection | null>(null);
const autonomous = ref(false);
const proposal = ref<Proposal | null>(null);
const bridge = ref<string | null>(null);
interface Activity {
  calls: number;
  last_tool: string | null;
  last_call_unix_ms: number | null;
  last_error: string | null;
}
const activity = ref<Activity | null>(null);
const now = ref(Date.now());
const emit = defineEmits<{ pending: [count: number] }>();
watch(proposal, (p) => emit("pending", p?.edits.length ?? 0));
let unlisten: UnlistenFn | undefined;
let poll: ReturnType<typeof setInterval> | undefined;
onMounted(async () => {
  if (!isTauri()) return;
  try {
    unlisten = await listen<Proposal | null>(
      "proposal",
      ({ payload }) => (proposal.value = payload),
    );
    proposal.value = await invoke("get_proposal");
    connection.value = await invoke("get_agent");
    bridge.value = await invoke("agent_bridge");
  } catch (error) {
    e.error = String(error);
  }
  // Live status while access is on: has an agent actually talked to us?
  poll = setInterval(async () => {
    now.value = Date.now();
    if (connection.value)
      activity.value = await invoke<Activity | null>("agent_activity").catch(
        () => null,
      );
  }, 1500);
});
onUnmounted(() => {
  unlisten?.();
  clearInterval(poll);
});
async function enable() {
  await e.guard(async () => {
    const root = await open({
      directory: true,
      multiple: false,
      title: "Folder the agent may import from and export into",
    });
    if (typeof root === "string")
      connection.value = await invoke("start_agent", {
        root,
        autonomous: autonomous.value,
      });
  });
}
async function disable() {
  await e.guard(async () => {
    await invoke("stop_agent");
    connection.value = null;
    activity.value = null;
  });
}
async function resolve(accept: boolean) {
  const p = proposal.value;
  if (!p) return;
  await e.guard(async () => {
    e.accept(
      await invoke<Snapshot>("resolve_proposal", { proposalId: p.id, accept }),
    );
    proposal.value = null;
  });
}

// "Address" snippets: MCP runs over stdio, so an agent needs a command, not a URL.
const bridgePath = computed(() => bridge.value ?? "/path/to/blyacut-mcp");
const quote = (s: string) => (/[\s"']/.test(s) ? JSON.stringify(s) : s);
const tabs = ["Claude Code", "JSON config", "Headless"] as const;
const tab = ref<(typeof tabs)[number]>("Claude Code");
const snippet = computed(() => {
  const config = connection.value?.config_path ?? "";
  const root = connection.value?.allowed_root ?? "/agent/folder";
  if (tab.value === "Claude Code")
    return `claude mcp add blyacut -- ${quote(bridgePath.value)} ${quote(config)}`;
  if (tab.value === "JSON config")
    return JSON.stringify(
      {
        mcpServers: {
          blyacut: { command: bridgePath.value, args: [config] },
        },
      },
      null,
      2,
    );
  return `${quote(bridgePath.value)} --headless ${quote(root)} --autonomous`;
});
const hint = computed(
  () =>
    ({
      "Claude Code": "Run it in a terminal, then restart Claude Code.",
      "JSON config":
        "Paste into your MCP settings: Claude Desktop, Cursor, Codex, Gemini CLI and others.",
      Headless:
        "The agent runs its own studio without this window, for fully autonomous editing.",
    })[tab.value],
);
const copied = ref(false);
async function copy() {
  try {
    await navigator.clipboard.writeText(snippet.value);
  } catch {
    const area = document.createElement("textarea");
    area.value = snippet.value;
    document.body.append(area);
    area.select();
    document.execCommand("copy");
    area.remove();
  }
  copied.value = true;
  setTimeout(() => (copied.value = false), 1500);
}
const status = computed(() => {
  const a = activity.value;
  if (!a?.calls) return { live: false, text: "Waiting for an agent…" };
  const ago = Math.max(
    0,
    Math.round((now.value - (a.last_call_unix_ms ?? 0)) / 1000),
  );
  return {
    live: ago < 30,
    text: `Agent connected · ${a.calls} call${a.calls === 1 ? "" : "s"} · ${a.last_tool} ${ago}s ago`,
  };
});
</script>
<template>
  <div class="agent">
    <header class="hero">
      <img :src="logo" alt="" width="40" height="40" />
      <div>
        <h3>Let an AI agent edit</h3>
        <p>
          Agents connect over MCP, the open standard used by Claude, Cursor,
          Codex and Gemini, and edit this timeline alongside you.
        </p>
      </div>
    </header>

    <ol class="steps">
      <li :class="{ done: connection }">
        <span class="n">1</span>
        <div>
          <strong>Agent folder</strong>
          <p v-if="!connection">
            The agent imports media from it and exports into it. It sees no
            other files.
          </p>
          <p v-else class="path" :title="connection.allowed_root">
            {{ connection.allowed_root }}
          </p>
          <label v-if="!connection" class="toggle"
            ><input v-model="autonomous" type="checkbox" /> Autonomous: delete
            and make large edits without asking me</label
          >
          <p v-else-if="connection.mode === 'autonomous'" class="autonomy-on">
            Autonomous mode. Undo (⌘Z) still works.
          </p>
          <button
            :class="connection ? 'ghost-danger' : 'primary'"
            :disabled="e.busy"
            @click="connection ? disable() : enable()"
          >
            {{ connection ? "Disconnect agent" : "Choose folder and connect…" }}
          </button>
        </div>
      </li>

      <li :class="{ dim: !connection }">
        <span class="n">2</span>
        <div class="address">
          <strong>Agent address</strong>
          <div class="tabs" role="tablist">
            <button
              v-for="t in tabs"
              :key="t"
              role="tab"
              :aria-selected="tab === t"
              @click="tab = t"
            >
              {{ t }}
            </button>
          </div>
          <pre class="snippet"><code>{{ snippet }}</code></pre>
          <div class="row">
            <span class="hint">{{ hint }}</span>
            <button
              class="copy"
              :disabled="!connection && tab !== 'Headless'"
              @click="copy"
            >
              {{ copied ? "Copied ✓" : "Copy" }}
            </button>
          </div>
          <p v-if="!bridge" class="warn">
            Bridge not found: build it with
            <code>cargo build -p blyacut-mcp</code>.
          </p>
        </div>
      </li>

      <li :class="{ dim: !connection }">
        <span class="n">3</span>
        <div>
          <strong>Ask your agent</strong>
          <p class="example">
            “Edit the files in my folder into a 30-second video with music and
            subtitles.”
          </p>
          <p v-if="connection" class="status" :class="{ live: status.live }">
            <i />{{ status.text }}
          </p>
          <p v-if="activity?.last_error" class="warn">
            Last agent error: {{ activity.last_error }}
          </p>
        </div>
      </li>
    </ol>

    <details class="explain">
      <summary>Why an “address” and not a URL?</summary>
      <p>
        An MCP agent starts the small <code>blyacut-mcp</code> bridge on this
        Mac and talks to it over standard input and output. The bridge finds
        this window through a private session file only you can read, so no
        network port opens and nobody outside can join. The session file changes
        every time you connect.
      </p>
    </details>

    <div v-if="proposal" class="proposal-card">
      <strong>The agent proposes edits</strong>
      <p>
        {{ proposal.edits.length }} edits · revision {{ proposal.revision }}
      </p>
      <p>
        Result length:
        {{
          time(
            Math.max(
              0,
              ...proposal.preview.tracks.flatMap((t) =>
                t.clips.map((c) => c.position_ms + clipLength(c)),
              ),
            ),
          )
        }}
      </p>
      <ol class="edit-review">
        <li v-for="(edit, index) in proposal.edits" :key="index">
          {{ describeEdit(edit, e.project) }}
        </li>
      </ol>
      <details>
        <summary>Technical details</summary>
        <pre>{{ JSON.stringify(proposal.edits, null, 2) }}</pre>
      </details>
      <button class="primary full" @click="resolve(true)">Apply edits</button
      ><button class="full" @click="resolve(false)">Reject</button>
    </div>
  </div>
</template>
<style scoped>
.agent {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 14px;
  font-size: 12px;
  color: var(--text-secondary);
}
.hero {
  display: flex;
  gap: 12px;
  align-items: flex-start;
}
.hero h3 {
  margin: 0 0 4px;
  font-size: 14px;
  color: var(--text-primary);
}
.hero p {
  margin: 0;
  line-height: 1.45;
}
.steps {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.steps > li {
  display: grid;
  grid-template-columns: 22px minmax(0, 1fr);
  gap: 10px;
}
.steps > li.dim {
  opacity: 0.55;
}
.n {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  display: grid;
  place-items: center;
  font-weight: 700;
  font-size: 11px;
  color: var(--accent-ink);
  background: var(--accent);
}
.done .n {
  background: #2c3a1d;
  color: var(--accent);
  border: 1px solid #62744d;
}
.steps strong {
  display: block;
  color: var(--text-primary);
  font-size: 12px;
  margin-bottom: 4px;
}
.steps p {
  margin: 0 0 8px;
  line-height: 1.45;
}
.path {
  font-family: var(--font-code);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.toggle {
  display: flex;
  gap: 8px;
  align-items: flex-start;
  margin-bottom: 8px;
  cursor: pointer;
}
.toggle input {
  accent-color: var(--accent);
  margin-top: 2px;
}
.autonomy-on {
  padding: 5px 8px;
  border-radius: 5px;
  border: 1px solid #5c4b1c;
  background: #2c2614;
  color: #f4c46b;
}
.ghost-danger {
  color: var(--danger);
}
.tabs {
  display: flex;
  gap: 2px;
  background: var(--surface-input);
  border-radius: 6px;
  padding: 2px;
  margin-bottom: 6px;
}
.tabs button {
  flex: 1;
  padding: 4px 2px;
  font-size: 11px;
  background: transparent;
  border: 0;
  color: var(--text-secondary);
}
.tabs button[aria-selected="true"] {
  background: var(--surface-raised);
  color: var(--text-primary);
}
.snippet {
  margin: 0;
  padding: 8px 10px;
  max-height: 140px;
  overflow: auto;
  border-radius: 6px;
  background: #0b0d10;
  border: 1px solid var(--border-subtle);
  color: #d8f6b4;
  font: 11px/1.5 var(--font-code);
  white-space: pre-wrap;
  word-break: break-all;
}
.row {
  display: flex;
  gap: 8px;
  align-items: center;
  justify-content: space-between;
  margin-top: 6px;
}
.hint {
  font-size: 11px;
  color: var(--text-muted);
}
.copy {
  flex-shrink: 0;
  padding: 4px 10px;
  font-size: 11px;
}
.example {
  font-style: italic;
  color: var(--text-primary);
}
.status {
  display: flex;
  gap: 8px;
  align-items: center;
}
.status i {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #6b7280;
  flex-shrink: 0;
}
.status.live {
  color: var(--accent);
}
.status.live i {
  background: var(--accent);
  box-shadow: 0 0 0 4px #c1f17b33;
}
.warn {
  color: #f0c693;
}
.explain summary {
  cursor: pointer;
  color: var(--text-secondary);
}
.explain p {
  margin: 8px 0 0;
  line-height: 1.5;
}
code {
  font-family: var(--font-code);
  color: var(--text-primary);
}
</style>
