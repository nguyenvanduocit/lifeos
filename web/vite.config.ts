import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// `npm run dev` serves the app; the backend (`lifeos web`) answers /link.json
// and the browser dials its WebTransport link directly, not through Vite.
const backend = process.env.LIFEOS_BACKEND ?? 'http://127.0.0.1:7420'

export default defineConfig({
  plugins: [vue()],
  server: { proxy: { '/link.json': backend } },
  build: { target: 'es2022' },
})
