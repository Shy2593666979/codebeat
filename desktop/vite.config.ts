import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
export default defineConfig({plugins:[vue()],server:{port:1420,strictPort:true,watch:{ignored:['**/target/**','**/.build-tools/**','**/src-tauri/**','**/crates/**','**/docs/**']}},clearScreen:false,test:{environment:'jsdom',include:['src/**/*.test.ts']}})
