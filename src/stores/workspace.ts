import { defineStore } from "pinia";
import { ref } from "vue";
import { api } from "@/bindings/commands";
import type { Workspace } from "@/bindings/types";

export const useWorkspaceStore = defineStore("workspace", () => {
  const current = ref<Workspace | null>(null);
  const recent = ref<Workspace[]>([]);
  const loading = ref(false);

  /** 启动时尝试自动加载上次工作目录（FR-001）。 */
  async function init() {
    loading.value = true;
    try {
      current.value = await api.getCurrentWorkspace();
      recent.value = await api.listRecentWorkspaces();
    } finally {
      loading.value = false;
    }
  }

  async function select(path: string) {
    current.value = await api.selectWorkspace(path);
    recent.value = await api.listRecentWorkspaces();
  }

  async function switchTo(path: string) {
    current.value = await api.switchWorkspace(path);
    recent.value = await api.listRecentWorkspaces();
  }

  return { current, recent, loading, init, select, switchTo };
});
