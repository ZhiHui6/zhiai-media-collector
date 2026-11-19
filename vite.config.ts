import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // WebView2 会在 target/ 下生成被占用的缓存文件，监视它会让 dev server 因 EBUSY 崩溃
      ignored: ["**/src-tauri/**", "**/target/**", "**/release/**"],
    },
  },
  build: {
    target: "chrome110",
  },
});