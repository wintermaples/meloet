import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022" },
  // The development and measuring hooks (ui/src/dev, gl/measure.ts, gl/profile.ts) are in the
  // page only when built with MELOET_DEV_HOOKS=1; the tests always have them.
  define: { __DEV_HOOKS__: JSON.stringify(process.env.MELOET_DEV_HOOKS === "1" || !!process.env.VITEST) },
  // "browser" lets component tests mount Svelte components (vitest would load the server build).
  resolve: process.env.VITEST ? { conditions: ["browser"] } : undefined,
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts", "tests/**/*.test.ts"],
    setupFiles: ["tests/setup.ts"],
  },
});
