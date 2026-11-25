import { createApp } from 'vue'
import App from './App.vue'
import { router } from './router'
import { start } from './core/link'
import { start as startAssistant } from './assistant'
import './style.css'

start()
startAssistant()
createApp(App).use(router).mount('#app')
