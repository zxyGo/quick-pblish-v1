<script setup lang="ts">
import { onMounted } from "vue";
import { useArticlesStore } from "@/stores/articles";
import type { ArticleSummary } from "@/bindings/types";
import { formatTime } from "@/services/format";

const articles = useArticlesStore();
const emit = defineEmits<{ select: [item: ArticleSummary] }>();

onMounted(() => articles.refresh());

function onSearch() {
  articles.refresh();
}
</script>

<template>
  <div class="h-full flex flex-col">
    <div class="flex gap-2 px-3 py-2">
      <t-input
        v-model="articles.keyword"
        placeholder="搜索标题/标签/正文"
        clearable
        @change="onSearch"
        @enter="onSearch"
      />
      <t-select v-model="articles.sortBy" class="w-32 shrink-0" @change="onSearch">
        <t-option value="updated" label="按修改时间" />
        <t-option value="created" label="按创建时间" />
        <t-option value="title" label="按标题" />
      </t-select>
    </div>

    <div class="flex-1 min-h-0 overflow-auto">
      <t-loading :loading="articles.loading" size="small" class="block">
        <template v-if="articles.list.length">
          <div
            v-for="item in articles.list"
            :key="item.relativePath"
            class="px-3 py-2.5 border-b border-gray-100 cursor-pointer hover:bg-gray-50 transition-colors"
            @click="emit('select', item)"
          >
            <div class="font-600 text-sm truncate">{{ item.title }}</div>
            <div
              v-if="item.tags.length"
              class="flex items-center gap-1 flex-wrap mt-1"
            >
              <t-tag
                v-for="t in item.tags"
                :key="t"
                size="small"
                variant="light"
                >{{ t }}</t-tag
              >
            </div>
            <div class="flex items-center justify-between gap-2 mt-1">
              <span class="text-xs secondary-text truncate">
                {{ item.excerpt || "（空文章）" }}
              </span>
              <span class="text-xs muted shrink-0">{{
                formatTime(item.updated)
              }}</span>
            </div>
          </div>
        </template>
        <t-empty v-else description="还没有文章" class="mt-8" />
      </t-loading>
    </div>
  </div>
</template>
