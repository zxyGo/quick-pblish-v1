<script setup lang="ts">
import { onMounted } from "vue";
import { useArticlesStore } from "@/stores/articles";
import type { ArticleSummary } from "@/bindings/types";

const articles = useArticlesStore();
const emit = defineEmits<{ select: [item: ArticleSummary] }>();

onMounted(() => articles.refresh());

function onSearch() {
  articles.refresh();
}
</script>

<template>
  <div class="h-full flex flex-col">
    <div class="flex gap-2 p-2">
      <t-input
        v-model="articles.keyword"
        placeholder="搜索标题/标签/正文"
        clearable
        @change="onSearch"
        @enter="onSearch"
      />
      <t-select v-model="articles.sortBy" size="small" @change="onSearch">
        <t-option value="updated" label="按修改时间" />
        <t-option value="created" label="按创建时间" />
        <t-option value="title" label="按标题" />
      </t-select>
    </div>

    <t-loading :loading="articles.loading" size="small">
      <t-list v-if="articles.list.length" size="small" :split="true">
        <t-list-item
          v-for="item in articles.list"
          :key="item.relativePath"
          class="flex flex-col items-start cursor-pointer"
          @click="emit('select', item)"
        >
          <div class="font-600">{{ item.title }}</div>
          <div class="flex gap-1.5 items-center my-1">
            <t-tag
              v-for="t in item.tags"
              :key="t"
              size="small"
              variant="light"
              >{{ t }}</t-tag
            >
            <span class="text-xs muted">{{ item.updated }}</span>
          </div>
          <div class="text-xs secondary-text truncate max-w-full">
            {{ item.excerpt }}
          </div>
        </t-list-item>
      </t-list>
      <t-empty v-else description="还没有文章" />
    </t-loading>
  </div>
</template>
