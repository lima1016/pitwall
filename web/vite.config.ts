/// <reference types="vitest/config" />
import react from '@vitejs/plugin-react'
import { defineConfig, type ProxyOptions } from 'vite'

// 개발 서버는 API를 pitwall 서버(127.0.0.1:7311)로 프록시한다.
// pitwall 서버는 Host와 Origin이 자기 주소인 요청만 받으므로 프록시에서 둘 다 바꿔 준다 (DESIGN.md 7장).
const backend = 'http://127.0.0.1:7311'

const toBackend: ProxyOptions = {
  target: backend,
  changeOrigin: true,
  configure: (proxy) => {
    proxy.on('proxyReq', (req) => req.setHeader('origin', backend))
    proxy.on('proxyReqWs', (req) => req.setHeader('origin', backend))
  },
}

export default defineConfig({
  plugins: [react()],
  server: {
    host: '127.0.0.1',
    proxy: {
      '/api': toBackend,
      '/hooks': toBackend,
      '/ws': { ...toBackend, ws: true },
    },
  },
  test: {
    environment: 'jsdom',
  },
})
