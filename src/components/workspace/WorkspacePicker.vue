<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { MessagePlugin } from "tdesign-vue-next";
import { useWorkspaceStore } from "@/stores/workspace";
import { toAppError } from "@/services/error";

const workspace = useWorkspaceStore();

async function pickFolder() {
  const selected = await open({ directory: true, multiple: false });
  if (typeof selected === "string") {
    try {
      await workspace.select(selected);
      MessagePlugin.success("已打开工作目录");
    } catch (e) {
      MessagePlugin.error(toAppError(e).message);
    }
  }
}

async function openRecent(path: string) {
  try {
    await workspace.switchTo(path);
  } catch (e) {
    MessagePlugin.error(toAppError(e).message);
  }
}
</script>

<template>
  <div class="p-3">
    <t-button theme="primary" @click="pickFolder">选择工作目录</t-button>
    <div v-if="workspace.recent.length" class="mt-4">
      <div class="text-xs muted mb-1">最近使用</div>
      <t-list size="small">
        <t-list-item
          v-for="r in workspace.recent"
          :key="r.path"
          class="flex flex-col cursor-pointer"
          @click="openRecent(r.path)"
        >
          <span class="font-600">{{ r.name }}</span>
          <span class="text-xs muted">{{ r.path }}</span>
        </t-list-item>
      </t-list>
    </div>
  </div>
</template>
