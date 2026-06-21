import { defineStore } from "pinia";
import { ref } from "vue";
import { api } from "@/bindings/commands";
import type { ArticleSummary, ListQuery } from "@/bindings/types";

export const useArticlesStore = defineStore("articles", () => {
  const list = ref<ArticleSummary[]>([]);
  const keyword = ref("");
  const sortBy = ref<NonNullable<ListQuery["sortBy"]>>("updated");
  const order = ref<NonNullable<ListQuery["order"]>>("desc");
  const loading = ref(false);

  async function refresh() {
    loading.value = true;
    try {
      list.value = await api.listArticles({
        keyword: keyword.value || undefined,
        sortBy: sortBy.value,
        order: order.value,
      });
    } finally {
      loading.value = false;
    }
  }

  return { list, keyword, sortBy, order, loading, refresh };
});
