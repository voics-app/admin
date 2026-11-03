/// <reference types="vitest/config" />
import react from "@vitejs/plugin-react";
import { defineConfig, loadEnv, type Plugin } from "vite";
import brand from "./src/brand.json";

/** Fills %BRAND_ADMIN_TITLE% in index.html from src/brand.json. */
const brandHtml: Plugin = {
  name: "brand-html",
  transformIndexHtml: (html) => html.replace("%BRAND_ADMIN_TITLE%", brand.adminTitle),
};

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const bff = env.ADMIN_BFF_URL ?? "http://localhost:8090";
  const devEmail = env.ADMIN_DEV_EMAIL ?? "dev@discordy.local";
  return {
    plugins: [react(), brandHtml],
    server: {
      port: 5174,
      proxy: {
        "/admin-api": {
          target: bff,
          headers: { "x-dev-admin-email": devEmail },
        },
      },
    },
    build: { outDir: "dist", sourcemap: false },
    test: {
      environment: "jsdom",
      globals: true,
      setupFiles: ["./src/test/setup.ts"],
      css: false,
    },
  };
});
