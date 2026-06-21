import { createRouter, createWebHistory } from "vue-router";
import MainView from "@/views/MainView.vue";

export const router = createRouter({
  history: createWebHistory(),
  routes: [{ path: "/", name: "main", component: MainView }],
});
