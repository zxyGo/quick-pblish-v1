import { defineConfig, presetWind3 } from "unocss";

export default defineConfig({
  presets: [presetWind3()],
  theme: {
    colors: {
      // 与 tdesign 主色对齐，便于原子类与组件库视觉统一
      brand: "#0052d9",
    },
  },
  shortcuts: {
    "muted": "text-gray-400",
    "secondary-text": "text-gray-500",
  },
});
