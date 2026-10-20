import {defineConfig} from '@playwright/test'
export default defineConfig({testDir:'./tests/ui',use:{channel:'chromium',headless:true,launchOptions:{args:['--no-proxy-server']},viewport:{width:1280,height:900}},reporter:[['list'],['html',{open:'never'}]]})
