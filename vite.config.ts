import path from "path";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,

  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 5173,
    strictPort: false,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1420,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["*/src-tauri/**"],
    },
  },

  // The 'root' tells Vite to use the 'frontend' directory
  // as the base for all its operations, including finding index.html
  root: path.resolve(__dirname, "./src-ui-frontend"),

  build: {
    // 'outDir' is relative to the new 'root' (i.e., 'frontend')
    // We use a relative path '../dist' to put the final build
    // *outside* the 'frontend' folder, typically at project-root/dist
    outDir: "../dist",
  },

  resolve: {
    alias: {
      "@audio-player": path.resolve(__dirname, "./src-ui-frontend/modules/audio-player"),
      "@audio-capture": path.resolve(__dirname, "./src-ui-frontend/modules/audio-capture"),
      "@ui": path.resolve(__dirname, "./src-ui-frontend"),
    },
  },
}));
