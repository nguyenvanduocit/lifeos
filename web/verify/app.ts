// The whole app (App.vue) on made-up data through a fake link (fakeLink.ts): no server. ?at=<path> opens a page, the
// Assistant opens with ?assistant=1 (add ?assistantPanel=plan|session|memory for a tab); ?as=idle has the Assistant at
// rest, ?still=1 stops the streaming, ?scenario=many|one|long|none picks the notes (notesData.ts), ?widgets=1|<n>|tall puts
// widgets on the Notes page. What the panel asked of the link is in window.calls.
import { createApp } from 'vue'
import App from '../src/App.vue'
import { router } from '../src/router'
import { start } from './fakeLink'
import { start as startAssistant } from '../src/assistant'
import '../src/style.css'

const params = new URLSearchParams(location.search)
start()
startAssistant()
const at = params.get('at') ?? '/'
const HARNESS = ['at', 'as', 'still', 'scenario', 'widgets']
const query = Object.fromEntries([...params].filter(([k]) => !HARNESS.includes(k)))
// The router's first move is to where the address bar is (/verify/app.html, which no page has): going to `at` before the
// app takes the router makes that the first move instead.
void router.replace({ path: at, query }).then(() => createApp(App).use(router).mount('#app'))
