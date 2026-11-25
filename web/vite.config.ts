import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

// `npm run dev` serves the app; the backend (`lifeos web`) answers /link.json
// and the browser dials its WebTransport link directly, not through Vite.
const backend = process.env.LIFEOS_BACKEND ?? 'http://127.0.0.1:7420'

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  server: { proxy: { '/link.json': backend } },
  build: { target: 'es2022' },
})
