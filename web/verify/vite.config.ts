// Serves the pages in this folder against the real components, with no backend: `npm run verify`, then
// http://127.0.0.1:5297/verify/app.html?at=/notes (or any page's own path, e.g. http://127.0.0.1:5297/notes: the app's
// index.html gets the fake link too). The config itself is in config.mjs.
import { defineConfig } from 'vite'
import { verifyConfig } from './config.mjs'

export default defineConfig(verifyConfig())
