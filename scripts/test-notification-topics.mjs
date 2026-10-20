// Isolated native EXE smoke test: no live model calls or reset-card use.
import {createRequire} from 'node:module'
const {chromium}=createRequire(new URL('../desktop/package.json',import.meta.url))('@playwright/test')
import {spawn} from 'node:child_process'
import {mkdir,writeFile} from 'node:fs/promises'
import path from 'node:path'
import net from 'node:net'
import {randomUUID} from 'node:crypto'
import assert from 'node:assert/strict'
const root=process.cwd(),temp=path.join(root,'.build-tools','notification-topics-'+randomUUID())
await mkdir(temp,{recursive:true})
await mkdir(path.join(root,'docs/logs'),{recursive:true})
const port=await new Promise(resolve=>{const server=net.createServer();server.listen(0,'127.0.0.1',()=>{const port=server.address().port;server.close(()=>resolve(port))})})
const exe=path.join(root,'target/release/codebeat.exe')
const env={...process.env,LOCALAPPDATA:temp,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`}
const app=spawn(exe,['--background'],{env,windowsHide:true,stdio:['ignore','ignore','pipe']})
let browser,stderr='';app.stderr.on('data',chunk=>stderr+=chunk)
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms))
async function until(check,label){for(let i=0;i<100;i++){try{const result=await check();if(result)return result}catch{}await delay(250)}throw Error('Timed out: '+label)}
try{
 await until(async()=>{const response=await fetch(`http://127.0.0.1:${port}/json/version`);return response.ok},'WebView2')
 browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`)
 const main=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri')&&!p.url().includes('/tray')),'main page')
 await main.waitForFunction(()=>!!window.__TAURI_INTERNALS__)
 const invoke=(command,args={})=>main.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args})
 assert.equal(await invoke('plugin:window|is_visible',{label:'main'}),false)
 let items=await invoke('get_pending_notices');assert.equal(items.length,0)
 const first=randomUUID(),second=randomUUID(),quota=randomUUID()
 await invoke('publish_notice',{id:first,topic:'provider:codex:scheduling',title:'Codex schedule test',message:'enabled',kind:'success'})
 await invoke('publish_notice',{id:second,topic:'provider:codex:scheduling',title:'Codex schedule test',message:'disabled',kind:'success'})
 await invoke('publish_notice',{id:quota,topic:'provider:codex:quota-exhausted',title:'Quota test',message:'separate event',kind:'info'})
 items=await invoke('get_pending_notices');assert.equal(items.length,2);assert.ok(!items.some(n=>n.id===first));assert.equal(items.find(n=>n.id===second).message,'disabled')
 await invoke('acknowledge_notices',{ids:[first]});assert.ok((await invoke('get_pending_notices')).some(n=>n.id===second))
 await invoke('ignore_all_notices');assert.equal((await invoke('get_pending_notices')).length,0)
 await delay(16000)
 assert.equal((await invoke('get_pending_notices')).length,0)
 const result={passed:true,isolated_data:temp,no_startup_demo_messages:true,replacement_by_topic:true,old_revision_ack_safe:true,ignore_all_works:true,no_refill_after_16_seconds:true,real_model_requests:false,reset_cards_used:false}
 await writeFile('docs/logs/notification-topics-native.json',JSON.stringify(result,null,2))
 console.log(JSON.stringify(result,null,2))
}finally{
 if(browser)await browser.close()
 app.kill()
 await writeFile('docs/logs/notification-topics-native-stderr.txt',stderr)
}
