import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    environment: 'jsdom',
    setupFiles: ['./src-ui/src/tests/test-setup.ts'],
    globals: true,
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      exclude: [
        'node_modules/',
        'src-ui/src/tests/test-setup.ts',
        'src-ui/**/*.d.ts',
        'src-ui/**/*.config.*',
        'dist/'
      ]
    }
  },
  resolve: {
    alias: {
      '@': '/src-ui'
    }
  }
})
