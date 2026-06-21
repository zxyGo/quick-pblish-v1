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
  <div class="w-full">
    <t-button theme="primary" block @click="pickFolder">选择工作目录</t-button>

    <div v-if="workspace.recent.length" class="mt-3">
      <div class="text-xs muted mb-1">最近使用</div>
      <div
        v-for="r in workspace.recent"
        :key="r.path"
        class="px-2 py-1.5 rounded cursor-pointer hover:bg-gray-50 transition-colors"
        @click="openRecent(r.path)"
      >
        <div class="text-sm font-600 truncate">{{ r.name }}</div>
        <div class="text-xs muted truncate" :title="r.path">{{ r.path }}</div>
      </div>
    </div>
  </div>
</template>
