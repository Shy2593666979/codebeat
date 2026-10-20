// Isolated native EXE smoke test: no live model calls or reset-card use.
import {createRequire} from 'node:module'
const {chromium}=createRequire(new URL('../desktop/package.json',import.meta.url))('@playwright/test')
import {spawn} from 'node:child_process'
import {mkdir,writeFile} from 'node:fs/promises'
import path from 'node:path'
import net from 'node:net'
import {randomUUID} from 'node:crypto'
import assert from 'node:assert/strict'
const root=process.cwd(),temp=path.join(root,'.build-tools','window-size-'+randomUUID())
await mkdir(temp,{recursive:true})
await mkdir(path.join(root,'docs/logs'),{recursive:true})
let port=await new Promise(resolve=>{const server=net.createServer();server.listen(0,'127.0.0.1',()=>{const port=server.address().port;server.close(()=>resolve(port))})})
const exe=process.env.CODEBEAT_TEST_EXE||path.join(root,'target/release/codebeat.exe')
const env={...process.env,LOCALAPPDATA:temp,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`}
const app=spawn(exe,['--open-page','usage'],{env,windowsHide:true,stdio:['ignore','ignore','pipe']})
let browser,stderr='';app.stderr.on('data',chunk=>stderr+=chunk)
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms))
async function until(check,label){for(let i=0;i<100;i++){try{const result=await check();if(result)return result}catch{}await delay(250)}throw Error('Timed out: '+label)}
try{
 await until(async()=>{const response=await fetch(`http://127.0.0.1:${port}/json/version`);return response.ok},'WebView2')
 browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`)
 const main=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>(p.url().includes('tauri')||p.url().includes('127.0.0.1:1420'))&&!p.url().includes('/tray')),'main page')
 await main.waitForFunction(()=>!!window.__TAURI_INTERNALS__)
 const invoke=(command,args={})=>main.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args})
 const layout=await invoke('get_display_layout'),dpi=await invoke('plugin:window|scale_factor',{label:'main'})
 console.log('Initial measured client:',await invoke('plugin:window|inner_size',{label:'main'}),'layout',layout,'dpi',dpi)
 const expected=()=>({width:Math.round(1200*layout.effective*dpi),height:Math.round(800*layout.effective*dpi)})
 await until(async()=>{const size=await invoke('plugin:window|inner_size',{label:'main'});return Math.abs(size.width-expected().width)<=1&&Math.abs(size.height-expected().height)<=1},'exact baseline client size')
 const first=await invoke('plugin:window|inner_size',{label:'main'})
 assert.equal(await invoke('plugin:window|is_visible',{label:'main'}),true)
 await invoke('save_window_layout')
 await browser.close();browser=null
 app.kill();await new Promise(resolve=>app.once('exit',resolve))
 const samples=[first]
 for(let cycle=0;cycle<2;cycle++){
  port=await new Promise(resolve=>{const server=net.createServer();server.listen(0,'127.0.0.1',()=>{const port=server.address().port;server.close(()=>resolve(port))})})
  env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=`--remote-debugging-port=${port}`
  const next=spawn(exe,['--open-page','usage'],{env,windowsHide:true,stdio:'ignore'})
  try{
   await until(async()=>{const response=await fetch(`http://127.0.0.1:${port}/json/version`);return response.ok},'reopened WebView2')
   browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`)
   const reopened=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>(p.url().includes('tauri')||p.url().includes('127.0.0.1:1420'))&&!p.url().includes('/tray')),'reopened main')
   await reopened.waitForFunction(()=>!!window.__TAURI_INTERNALS__)
   const read=command=>reopened.evaluate(command=>window.__TAURI_INTERNALS__.invoke(command,{label:'main'}),command)
   await until(async()=>{const size=await read('plugin:window|inner_size');return Math.abs(size.width-first.width)<=1&&Math.abs(size.height-first.height)<=1},'stable reopened client size')
   samples.push(await read('plugin:window|inner_size'))
   await read('save_window_layout')
  }finally{
   if(browser){await browser.close();browser=null}
   next.kill();await new Promise(resolve=>next.once('exit',resolve))
  }
 }
 const result={passed:true,isolated_data:temp,dpi,effective_scale:layout.effective,expected:expected(),client_sizes:samples,reopen_cycles:2,real_model_requests:false,reset_cards_used:false}
 await writeFile('docs/logs/window-size-native.json',JSON.stringify(result,null,2))
 console.log(JSON.stringify(result,null,2))
}finally{
 if(browser)await browser.close()
 app.kill()
 await writeFile('docs/logs/window-size-native-stderr.txt',stderr)
}
