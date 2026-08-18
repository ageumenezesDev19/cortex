import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
// @ts-expect-error process is a nodejs global
const demo = process.env.VITE_DEMO === "1";

export default defineConfig(async () => ({
  plugins: [react()],

  /**
   * Transformers.js is the demo's alone — the desktop embeds in Rust. Without
   * this the 22 MB ONNX runtime lands inside the .app, which would ship a
   * WebAssembly build of something the app already does natively.
   *
   * In the demo it is aliased too, because Transformers.js fetches its runtime
   * from a CDN at load time and never reads the copy Vite emits. Measured: the
   * bundled file got zero requests while the jsdelivr one got two.
   */
  resolve: demo ? {} : { alias: { "@huggingface/transformers": "/src/demo/absent.ts" } },


  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
