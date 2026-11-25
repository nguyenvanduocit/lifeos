// Serves the pages in this folder against the real components, with no backend: `npm run verify`, then
// http://127.0.0.1:5297/verify/app.html. The page's link (src/core/link.ts) is swapped for a fake (fakeLink.ts) that
// answers from made-up data and streams a message, as remit's verify pages do.
import path from 'node:path'
import { defineConfig, type Plugin } from 'vite'
import tailwindcss from '@tailwindcss/vite'
import vue from '@vitejs/plugin-vue'

const web = path.resolve(import.meta.dirname, '..')
const real = path.join(web, 'src/core/link.ts')
const fake = path.join(web, 'verify/fakeLink.ts')
const swapLink: Plugin = {
  name: 'swap-link',
  enforce: 'pre',
  async resolveId(source, importer, options) {
    if (!importer || importer === fake || !source.endsWith('core/link')) return null
    const resolved = await this.resolve(source, importer, { ...options, skipSelf: true })
    return resolved?.id === real ? fake : null
  },
}

export default defineConfig({
  root: web,
  plugins: [swapLink, vue(), tailwindcss()],
  server: { port: 5297, strictPort: true, host: '127.0.0.1' },
})
