/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },

  test: {
    environment: "jsdom",
    // 组件交互测试（@testing-library）需要 DOM；纯函数/契约测试不受影响
    include: ["src/**/*.{test,spec}.{js,ts,tsx}"],
  },
});
