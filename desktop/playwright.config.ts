import {defineConfig} from '@playwright/test'
export default defineConfig({
 testDir:'./tests/ui',
 // 宣传截图按需在本地生成；CI 只运行功能回归。
 testIgnore:process.env.CI?['**/readme-gallery.spec.ts']:[],
 use:{channel:'chromium',headless:true,launchOptions:{args:['--no-proxy-server']},viewport:{width:1280,height:900}},
 reporter:[['list'],['html',{open:'never'}]]
})
