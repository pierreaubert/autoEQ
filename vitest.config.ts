import { defineConfig } from "vitest/config";
import path from "path";

export default defineConfig({
  test: {
    environment: "jsdom",
    setupFiles: ["./src-ui-frontend/tests/test-setup.ts"],
    globals: true,
    include: [
      "./src-ui-frontend/tests/**/*.{test,spec}.{js,ts}",
    ],
    coverage: {
      provider: "v8",
      reporter: ["text", "json", "html"],
      exclude: [
        "node_modules/",
        "./src-ui-frontend/tests/test-setup.ts",
        "./src-ui-frontend/**/*.d.ts",
        "./src-ui-frontend/**/*.config.*",
        "./dist/"
      ],
    },
  },
  resolve: {
    alias: {
      "@": "/src-ui-frontend",
      "@audio-player": path.resolve(__dirname, "./src-ui-frontend/modules/audio-player"),
      "@audio-capture": path.resolve(__dirname, "./src-ui-frontend/modules/audio-capture"),
      "@ui": path.resolve(__dirname, "./src-ui-frontend"),
    },
  },
});
