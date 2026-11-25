import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

// `npm run dev` serves the app and proxies link and account requests
// and the browser dials its WebTransport link directly, not through Vite.
const backend = process.env.LIFEOS_BACKEND ?? 'http://127.0.0.1:7420'

export default defineConfig({
  resolve: { alias: { '@': new URL('./src', import.meta.url).pathname } },
  plugins: [vue(), tailwindcss()],
  server: { proxy: { '/link.json': backend, '/api/attachments': backend, '/api/accounts': backend, '/api/plugins': backend, '/plugin-assets': backend, '/api/vault': backend, '/api/widgets': backend, '/api/google': backend } },
  build: { target: 'es2022' },
})
