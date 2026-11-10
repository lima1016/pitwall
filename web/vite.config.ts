import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

// 개발 서버는 API를 pitwall 서버(127.0.0.1:7311)로 프록시한다.
const backend = 'http://127.0.0.1:7311'

export default defineConfig({
  plugins: [react()],
  server: {
    host: '127.0.0.1',
    proxy: {
      '/api': backend,
      '/hooks': backend,
      '/ws': { target: backend, ws: true },
    },
  },
  test: {
    environment: 'jsdom',
  },
})
