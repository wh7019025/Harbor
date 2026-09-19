import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";

const backendPort = (globalThis as typeof globalThis & {
  process?: { env?: Record<string, string | undefined> };
}).process?.env?.HARBOR_PANEL_INTERFACE_PORT;
const backendUrl = backendPort ? `http://127.0.0.1:${backendPort}` : undefined;

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  server: {
    port: 18081,
    proxy: backendUrl ? {
      "/api": backendUrl,
      "/ws": {
        target: backendUrl,
        ws: true,
      },
    } : undefined,
  },
});
