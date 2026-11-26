// The Vite config of the verify harness, as plain JavaScript so both `npm run verify` (vite.config.ts) and the headless
// checks (notes.mjs, which starts Vite itself and so writes no bundled config file) share it. The page's link
// (src/core/link.ts) is swapped for a fake (fakeLink.ts) that answers from made-up data, as remit's verify pages do.
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import tailwindcss from '@tailwindcss/vite'
import vue from '@vitejs/plugin-vue'

export const web = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const real = path.join(web, 'src/core/link.ts')
const fake = path.join(web, 'verify/fakeLink.ts')
const swapLink = {
  name: 'swap-link',
  enforce: 'pre',
  async resolveId(source, importer, options) {
    if (!importer || importer === fake || !source.endsWith('core/link')) return null
    const resolved = await this.resolve(source, importer, { ...options, skipSelf: true })
    return resolved?.id === real ? fake : null
  },
}

export function verifyConfig() {
  return {
    root: web,
    resolve: { alias: { '@': path.join(web, 'src') } },
    plugins: [swapLink, vue(), tailwindcss()],
    // Where `npm run verify` serves a person at a browser. The checks (notes.mjs, contract.mjs) start no server.
    server: { port: 5297, strictPort: true, host: '127.0.0.1' },
  }
}
