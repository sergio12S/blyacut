<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import { describeEdit } from "../edit-description";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useEditor } from "../stores/editor";
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
const emit = defineEmits<{ pending: [count: number] }>();
watch(proposal, (p) => emit("pending", p?.edits.length ?? 0));
let unlisten: UnlistenFn | undefined;
onMounted(async () => {
  if (!isTauri()) return;
  try {
    unlisten = await listen<Proposal | null>(
      "proposal",
      ({ payload }) => (proposal.value = payload),
    );
    proposal.value = await invoke("get_proposal");
    connection.value = await invoke("get_agent");
  } catch (error) {
    e.error = String(error);
  }
});
onUnmounted(() => unlisten?.());
async function enable() {
  await e.guard(async () => {
    const root = await open({
      directory: true,
      multiple: false,
      title: "Choose the folder agents may import from and export into",
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
</script>
<template>
  <div class="agent-card">
    <span class="badge">MCP · LOCAL AGENT</span>
    <h3>
      {{ connection ? "Agent access enabled" : "Connect your editing agent" }}
    </h3>
    <p v-if="!connection">
      Choose a folder to allow media imports and new exports. External agents
      can edit this timeline through the MCP bridge.
    </p>
    <label v-if="!connection" class="autonomy-toggle"
      ><input v-model="autonomous" type="checkbox" /> Autonomous agent: allow
      deletions and large batches without my review</label
    >
    <p v-else-if="connection.mode === 'autonomous'" class="autonomy-on">
      Autonomous: the agent edits without review. Undo still works.
    </p>
    <details v-else :open="!proposal" class="connection-details">
      <summary>Connection details</summary>
      <p>
        Authorized folder:
        <strong class="path-label">{{ connection.allowed_root }}</strong>
      </p>
      <label
        >Session config<input
          readonly
          :value="connection.config_path"
          aria-label="Agent session config path"
          @focus="($event.target as HTMLInputElement).select()"
      /></label>
      <p class="muted">
        Pass this path to blyacut-mcp. Stop access when finished. To work
        without this app, run blyacut-mcp --headless FOLDER --autonomous.
      </p>
    </details>
    <button
      class="full"
      :disabled="e.busy"
      @click="connection ? disable() : enable()"
    >
      {{ connection ? "Stop agent access" : "Enable agent access…" }}
    </button>
    <div v-if="proposal" class="proposal-card">
      <strong>Review agent proposal</strong>
      <p>
        {{ proposal.edits.length }} edits · revision {{ proposal.revision }}
      </p>
      <p>
        Result:
        {{
          time(
            Math.max(
              0,
              ...proposal.preview.tracks.flatMap((t) =>
                t.clips.map(
                  (c) => c.position_ms + c.source_end_ms - c.source_start_ms,
                ),
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
      <button class="primary full" @click="resolve(true)">
        Apply proposed edits</button
      ><button class="full" @click="resolve(false)">Reject proposal</button>
    </div>
    <p class="muted">
      Transcription and media generation are not implemented yet.
    </p>
  </div>
</template>
<style scoped>
.autonomy-toggle {
  display: flex;
  gap: 8px;
  align-items: flex-start;
  margin: 8px 0 10px;
  font-size: 12px;
  line-height: 1.4;
  cursor: pointer;
}
.autonomy-toggle input {
  margin-top: 2px;
  accent-color: #c1f17b;
}
.autonomy-on {
  margin: 8px 0;
  padding: 6px 8px;
  border-radius: 5px;
  border: 1px solid #5c4b1c;
  background: #2c2614;
  color: #f4c46b;
  font-size: 12px;
}
</style>
