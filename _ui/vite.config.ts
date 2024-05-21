import { defineConfig } from 'vite';
import checker from 'vite-plugin-checker';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    checker({
      typescript: true,
      eslint: {
        lintCommand: 'eslint --ext .ts,.html . --ignore-path .gitignore',
      },
    }),
  ]
});
