import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    // Bound DOM worker contention and allow complete multi-step UI workflows on native CI.
    maxWorkers: 2,
    testTimeout: 15_000,
    setupFiles: ["./src/test/setup.ts"],
    clearMocks: true,
    restoreMocks: true,
  },
});
