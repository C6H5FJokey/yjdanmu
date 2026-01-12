import { createApp } from 'vue'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import App from './App.vue'
import PreviewWindow from './components/PreviewWindow.vue'

const app = createApp(App)
app.use(ElementPlus)
app.component('PreviewWindow', PreviewWindow)
app.mount('#app')