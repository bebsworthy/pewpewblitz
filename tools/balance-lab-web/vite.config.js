import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, ".", "");
  const target = env.BRAWLER_BALANCE_LAB_PROXY;

  return {
    plugins: [react()],
    server: {
      host: "127.0.0.1",
      proxy: target ? { "/api": { target, changeOrigin: false } } : undefined,
    },
  };
});
