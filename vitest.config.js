import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// Tests for the shared UI primitives live next to them under ui/.
export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    include: ['ui/**/*.test.{js,jsx}'],
  },
});
