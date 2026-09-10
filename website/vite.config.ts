import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig, loadEnv } from 'vite';

export default defineConfig(({ mode }) => {
  // The site is destined for GitHub Pages. A project site is served from
  // `/<repo>/`, a custom domain from the root, and the difference is baked into
  // every asset URL at build time — so it is an environment variable rather than
  // a value edited in this file before each deploy.
  const env = loadEnv(mode, process.cwd(), '');

  return {
    base: env.VITE_BASE_PATH || '/',
    plugins: [react(), tailwindcss()],
    build: {
      // Every visitor loads the whole page, so splitting it into chunks buys
      // nothing and costs a round trip.
      assetsInlineLimit: 2048,
    },
  };
});
