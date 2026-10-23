import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  base: './',
  plugins: [react()],
  resolve: { dedupe: ['react', 'react-dom'] },
  server: { host: '127.0.0.1', port: 1420, strictPort: true },
  build: { target: 'es2022' },
  test: { include: ['tests/**/*.test.ts'], environment: 'node', clearMocks: true },
});
