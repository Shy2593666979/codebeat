// Native retry integration with fixture CLIs only; no real credentials, model turns or cards.
import {createRequire} from 'node:module'
const {chromium}=createRequire(new URL('../desktop/package.json',import.meta.url))('@playwright/test')
import {spawn} from 'node:child_process'
import {mkdir,writeFile,readFile,copyFile,rename} from 'node:fs/promises'
import path from 'node:path'
import net from 'node:net'
import {randomUUID} from 'node:crypto'
import assert from 'node:assert/strict'
const root=process.cwd(),temp=path.join(root,'.build-tools','usage-retry-'+randomUUID()),bin=path.join(temp,'bin')
await mkdir(bin,{recursive:true})
for(const name of ['codex.exe','claude.exe'])await copyFile(path.join(root,'target/debug/codebeat-protocol-fixture.exe'),path.join(bin,name))
const control=path.join(temp,'control.json'),rpcLog=path.join(temp,'rpc.log')
const configure=async values=>{await writeFile(control+'.tmp',JSON.stringify(values));await rename(control+'.tmp',control)}
await configure({fail:false,used_percent:17,delay_ms:100})
const port=await new Promise(resolve=>{const server=net.createServer();server.listen(0,'127.0.0.1',()=>{const port=server.address().port;server.close(()=>resolve(port))})})
const env={...process.env,LOCALAPPDATA:temp,PATH:bin+path.delimiter+process.env.PATH,CODEBEAT_FIXTURE_USAGE_CONTROL:control,CODEBEAT_FIXTURE_RPC_LOG:rpcLog,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`}
const app=spawn(process.env.CODEBEAT_TEST_EXE||path.join(root,'target/debug/codebeat.exe'),['--background'],{env,windowsHide:true,stdio:['ignore','ignore','pipe']})
let browser,stderr='';app.stderr.on('data',s=>stderr+=s)
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms))
async function until(check,label){for(let i=0;i<150;i++){try{const result=await check();if(result)return result}catch{}await delay(200)}throw Error('Timed out: '+label)}
const methods=async()=>(await readFile(rpcLog,'utf8').catch(()=>'' )).trim().split(/\r?\n/).filter(Boolean)
const count=async()=>(await methods()).filter(m=>m==='account/rateLimits/read').length
try{
 await until(async()=>(await fetch(`http://127.0.0.1:${port}/json/version`)).ok,'WebView2')
 browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`)
 const main=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>(p.url().includes('tauri')||p.url().includes('127.0.0.1:1420'))&&!p.url().includes('/tray')),'main WebView')
 await main.waitForFunction(()=>!!window.__TAURI_INTERNALS__)
 const invoke=(command,args={})=>main.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args})
 const state=async()=>(await invoke('get_usage_refresh')).find(s=>s.provider==='codex')
 await until(async()=>{const s=await state();return !s.refreshing&&s.report?.status==='available'},'initial fixture quota')
 assert.equal(await invoke('plugin:window|is_visible',{label:'main'}),false)
 const before=await count()
 await configure({fail:false,used_percent:17,delay_ms:750})
 await Promise.all(Array.from({length:4},()=>invoke('refresh_usage',{id:'codex'})))
 assert.equal(await count()-before,1,'concurrent refreshes must share one RPC request')
 await configure({fail:true,delay_ms:100})
 const raw=await invoke('refresh_usage',{id:'codex'})
 assert.equal(raw.status,'unavailable')
 const failed=await state();assert.equal(failed.retrying,true);assert.equal(failed.report.status,'available');assert.equal(failed.report.windows[0].used_percent,17)
 assert.ok(failed.next_retry_at&&new Date(failed.next_retry_at)<new Date(failed.next_check_at))
 const monitoring=await invoke('get_monitoring',{days:1})
 assert.ok(monitoring.snapshots.some(s=>s.report.provider==='codex'&&s.report.status==='unavailable'))
 const tray=await invoke('get_tray_snapshot');assert.equal(tray.usage.find(r=>r?.provider==='codex').windows[0].used_percent,17)
 assert.equal((await invoke('get_pending_notices')).length,0,'read failures must not spam notifications')
 const failedCount=await count()
 await until(async()=>await count()>=failedCount+1,'automatic retry while the window is hidden')
 await until(async()=>!(await state()).refreshing,'first retry finishes')
 assert.equal((await state()).retrying,true)
 await configure({fail:false,used_percent:31,delay_ms:100})
 await until(async()=>{const s=await state();return !s.refreshing&&!s.retrying&&s.report?.windows[0]?.used_percent===31},'automatic retry recovers')
 const recoveredCount=await count();await delay(8000)
 assert.equal(await count(),recoveredCount,'successful recovery must cancel retries')
 assert.ok(!(await methods()).some(m=>m.includes('consume')||m.startsWith('turn/')))
 const report={passed:true,fixture_clis_only:true,isolated_data:temp,concurrent_requests:4,quota_reads_for_concurrent_group:1,failure_retains_last_success:true,raw_failure_is_persisted:true,tray_retains_last_success:true,retries_while_hidden:true,automatic_recovery:true,retries_stop_after_success:true,no_error_notification_spam:true,real_model_requests:false,reset_cards_used:false}
 await writeFile('docs/logs/usage-retry-native.json',JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2))
}finally{
 if(browser)await browser.close()
 const exited=new Promise(resolve=>app.once('exit',resolve));app.kill();await exited
 await writeFile('docs/logs/usage-retry-native-stderr.txt',stderr)
}
