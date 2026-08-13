import { defineConfig } from 'vite';
import dioxus from '@dioxus/devtools';

export default defineConfig({
  plugins: [dioxus()],
  resolve: {
    alias: {
      '@': '/src',
    },
  },
});