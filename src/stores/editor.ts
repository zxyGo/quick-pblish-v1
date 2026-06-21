import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { api } from "@/bindings/commands";
import type { ArticleContent, SaveArticleInput } from "@/bindings/types";
import { isConflict, toAppError } from "@/services/error";

export interface SaveResult {
  ok: boolean;
  conflict?: boolean;
  error?: string;
}

export const useEditorStore = defineStore("editor", () => {
  const open = ref<ArticleContent | null>(null);
  const title = ref("");
  const tags = ref<string[]>([]);
  const body = ref("");
  const baseHash = ref("");
  const dirty = ref(false);

  const hasOpen = computed(() => open.value !== null);

  function load(article: ArticleContent) {
    open.value = article;
    title.value = article.title;
    tags.value = [...article.tags];
    body.value = article.body;
    baseHash.value = article.baseHash;
    dirty.value = false;
  }

  function markDirty() {
    dirty.value = true;
  }

  function close() {
    open.value = null;
    title.value = "";
    tags.value = [];
    body.value = "";
    baseHash.value = "";
    dirty.value = false;
  }

  async function save(
    onConflict: SaveArticleInput["onConflict"] = "abort",
    saveAsPath?: string,
  ): Promise<SaveResult> {
    if (!open.value) return { ok: false, error: "没有打开的文章" };
    try {
      const saved = await api.saveArticle({
        relativePath: open.value.relativePath,
        title: title.value,
        tags: tags.value,
        body: body.value,
        baseHash: baseHash.value,
        onConflict,
        saveAsPath,
      });
      load(saved);
      return { ok: true };
    } catch (e) {
      if (isConflict(e)) return { ok: false, conflict: true };
      return { ok: false, error: toAppError(e).message };
    }
  }

  return {
    open,
    title,
    tags,
    body,
    baseHash,
    dirty,
    hasOpen,
    load,
    markDirty,
    close,
    save,
  };
});
