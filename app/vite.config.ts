import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { nodePolyfills } from "vite-plugin-node-polyfills";

export default defineConfig({
  plugins: [
    react(),
    nodePolyfills({
      include: ["buffer", "process"],
      globals: { Buffer: true, global: true, process: true },
    }),
  ],
  server: { port: 5173, host: true },
  preview: { port: 4173, host: true },
});
