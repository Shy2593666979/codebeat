// 测试真实 WebView2 通知进程；使用隔离配置和确认文件，不发送模型请求。
import {spawn} from 'node:child_process'
import {mkdtemp,mkdir,writeFile,appendFile,rm,stat} from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import net from 'node:net'
import assert from 'node:assert/strict'
import {randomUUID} from 'node:crypto'
import {createRequire} from 'node:module'
const require=createRequire(new URL('../desktop/package.json',import.meta.url))
const {chromium}=require('@playwright/test')
const executable=path.resolve(process.argv[2]||'target/debug/codebeat.exe')
const testData=await mkdtemp(path.join(os.tmpdir(),'codebeat-notice-data-test-'))
const ackDirectory=path.join(testData,'codebeat','notification-acks')
await mkdir(ackDirectory,{recursive:true})
await mkdir('docs/screenshots/notifications',{recursive:true})
await mkdir('docs/logs',{recursive:true})
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms))
async function port(){const server=net.createServer();await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const result=server.address().port;await new Promise(resolve=>server.close(resolve));return result}
async function exists(file){try{await stat(file);return true}catch{return false}}
async function helper(kind='success',page='tasks',queued=false){
 const id=randomUUID(),profile=await mkdtemp(path.join(os.tmpdir(),'codebeat-notice-test-')),debugPort=await port()
 const ack=path.join(ackDirectory,id+'.read'),dismiss=path.join(ackDirectory,id+'.dismiss')
 // 不依赖 WEBVIEW2_* 覆盖：管理员权限下需由测试构建通过 API 配置端口和目录。
 const env={...process.env,LOCALAPPDATA:testData,CODEBEAT_NOTIFICATION_TEST_PORT:String(debugPort),CODEBEAT_NOTIFICATION_TEST_PROFILE:profile}
 delete env.WEBVIEW2_USER_DATA_FOLDER;delete env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
 const started=Date.now(),child=spawn(executable,['--notification-helper'],{windowsHide:true,env,stdio:['pipe','pipe','pipe']})
 let stdout='',stderr='',readyAt,spawnError,lastConnectionError=''
 child.stdout.on('data',b=>{stdout+=b;if(readyAt===undefined&&/(^|\r?\n)READY\r?\n/.test(stdout))readyAt=Date.now()})
 child.stderr.on('data',b=>stderr+=b)
 child.once('error',error=>spawnError=error);child.stdin.on('error',error=>spawnError??=error)
 child.stdin.end(JSON.stringify({title:'CodeBeat 本地通知测试',body:'<script>&" 中文 $(literal)',kind,page,task_id:null,sound:false,ack_path:ack,dismiss_path:dismiss}))
 let browser
 const details=()=>({id,pid:child.pid,executable,queued,elapsed_ms:Date.now()-started,ready_ms:readyAt===undefined?null:readyAt-started,exit_code:child.exitCode,signal:child.signalCode,spawn_error:spawnError?.message,last_connection_error:lastConnectionError,stdout,stderr})
 function checkProcess(){
  if(spawnError)throw spawnError
  if(child.exitCode!==null||child.signalCode!==null)throw Error(`Helper exited before connection: ${JSON.stringify(details())}`)
  if(readyAt!==undefined&&!queued&&!stdout.includes('NOTIFICATION_TEST_CDP'))throw Error('Native test binary must be built with --features tauri/custom-protocol,notification-test')
 }
 async function connect(){
  checkProcess()
  try{
   const response=await fetch(`http://127.0.0.1:${debugPort}/json/version`,{signal:AbortSignal.timeout(1000)})
   if(!response.ok)throw Error(`DevTools discovery returned HTTP ${response.status}`)
   browser=await chromium.connectOverCDP(`http://127.0.0.1:${debugPort}`,{timeout:1500})
  }catch(error){lastConnectionError=error.message+(error.cause?.message?' / '+error.cause.message:'')}
  if(!browser)await delay(100)
 }
 async function checkReady(){
  while(readyAt===undefined&&Date.now()<deadline){checkProcess();await delay(50)}
  assert(readyAt!==undefined,`Helper never reported READY: ${JSON.stringify(details())}`)
  // 计时以收到 READY 为准，CDP 连接耗时不属于执行器的就绪承诺。
  assert(readyAt-started<8000,`Helper exceeded runner readiness timeout: ${JSON.stringify(details())}`)
 }
 async function cleanup(){
  child.kill();if(browser)await browser.close();await delay(250)
  for(const file of [ack,dismiss])await rm(file,{force:true})
  assert.equal(path.dirname(path.resolve(profile)),path.resolve(os.tmpdir()))
  assert(path.basename(profile).startsWith('codebeat-notice-test-'))
  await rm(profile,{recursive:true,force:true,maxRetries:10,retryDelay:200}).catch(()=>{})
 }
 const deadline=Date.now()+20000
 try{
  if(queued){
   await checkReady()
   return {child,ack,dismiss,cleanup,async rendered(){
    const promotedDeadline=Date.now()+20000
    while(!browser&&Date.now()<promotedDeadline)await connect()
    assert(browser,`Queued helper never constructed its WebView after slot release: ${JSON.stringify(details())}`)
    let view
    while(!view&&Date.now()<promotedDeadline){view=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('#/notification'));if(!view)await delay(100)}
    assert(view,'Queued helper never acquired a window');await view.locator('.native-notification').waitFor();return view
   }}
  }
  while(!browser&&Date.now()<deadline)await connect()
  assert(browser,`No WebView2 debugging endpoint: ${JSON.stringify(details())}`)
  let view
  while(!view&&Date.now()<deadline){view=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('#/notification'));if(!view)await delay(100)}
  assert(view,`No notification webview: ${stderr}`)
  // 验证“未交互”状态时隔离桌面上的真实鼠标输入，已读测试前再恢复事件。
  await view.evaluate(()=>{
   window.__codebeatBlockEngagement=true
   for(const type of ['mouseenter','mousemove','focusin'])window.addEventListener(type,event=>{
    if(window.__codebeatBlockEngagement)event.stopImmediatePropagation()
   },true)
  })
  await view.mouse.move(0,0)
  await view.locator('.native-notification').waitFor()
  await checkReady()
  await appendFile('docs/logs/notification-helper.jsonl',JSON.stringify({event:'connected',...details()})+'\n')
  return {child,view,ack,dismiss,profile,browser,cleanup}
 }catch(error){await appendFile('docs/logs/notification-helper.jsonl',JSON.stringify({event:'failed',error:error.message,...details()})+'\n');await cleanup();throw error}
}
async function waitExit(child){if(child.exitCode!==null)return;await Promise.race([new Promise(resolve=>child.once('exit',resolve)),delay(3000)])}
try {
// 上次测试可能把指针留在通知位置，先移到透明留白处，再验证未读保留。
const probe=await helper('info')
try{await probe.view.mouse.move(0,0);await writeFile(probe.dismiss,'');await waitExit(probe.child)}finally{await probe.cleanup()}
const success=await helper()
try{
 assert.equal(await success.view.locator('.native-content p').textContent(),'<script>&" 中文 $(literal)')
 assert.equal(await success.view.locator('.native-open').textContent(),'查看任务')
 await delay(2500)
 assert.equal(success.child.exitCode,null,'Unread notification closed automatically')
 assert.equal(await exists(success.ack),false,'Unread notification acknowledged itself')
 await success.view.screenshot({path:'docs/screenshots/notifications/rust-helper-success.png'})
 await success.view.evaluate(()=>window.__codebeatBlockEngagement=false)
 await success.view.locator('.native-notification').hover()
 await success.view.waitForSelector('.native-countdown')
 assert(await exists(success.ack),'Seen notification did not write acknowledgement')
 await delay(1100)
 await success.view.locator('.native-notification').hover()
 assert.equal(success.child.exitCode,null,'Read notification closed before two seconds')
 await success.view.mouse.move(0,0)
 await waitExit(success.child)
 assert.equal(success.child.exitCode,0,'Read notification did not exit cleanly')
}finally{await success.cleanup()}
const info=await helper('info','usage')
try{
 assert.equal(await info.view.locator('.native-open').textContent(),'查看额度')
 await info.view.screenshot({path:'docs/screenshots/notifications/rust-helper-info.png'})
 await writeFile(info.dismiss,'');await waitExit(info.child)
 assert.equal(info.child.exitCode,0,'External dismissal failed')
 assert.equal(await exists(info.dismiss),false,'Dismiss marker was not consumed')
}finally{await info.cleanup()}
const failure=await helper('error')
let slots=0
try{
 slots=await failure.view.evaluate(()=>Math.max(1,Math.floor((screen.availHeight-24)/200)))
 await failure.view.screenshot({path:'docs/screenshots/notifications/rust-helper-error.png'})
 await failure.view.locator('.native-close').dispatchEvent('click');await waitExit(failure.child)
 assert.equal(failure.child.exitCode,0,'Close button did not exit helper')
 assert(await exists(failure.ack),'Close button did not acknowledge notice')
}finally{await failure.cleanup()}
// 占满实际显示位置，验证溢出排队、补位及排队期间取消。
// 对特别高的屏幕限制压力测试规模。
if(slots<=7){
 const active=[]
 let queued,cancelled
 try{
  for(let i=0;i<slots;i++)active.push(await helper())
  queued=await helper('info','tasks',true)
  cancelled=await helper('info','tasks',true)
  await writeFile(cancelled.dismiss,'');await waitExit(cancelled.child)
  assert.equal(cancelled.child.exitCode,0,'Queued dismiss did not exit')
  await writeFile(active[0].dismiss,'');await waitExit(active[0].child)
  const view=await queued.rendered()
  assert.equal(queued.child.exitCode,null,'Queued notification exited before interaction')
  await view.screenshot({path:'docs/screenshots/notifications/rust-helper-queued.png'})
  await writeFile(queued.dismiss,'');await waitExit(queued.child)
  assert.equal(queued.child.exitCode,0,'Promoted notification could not close')
 }finally{for(const item of [...active,queued,cancelled].filter(Boolean))await item.cleanup()}
}
console.log('Rust notification helper: WebView2 rendering, literal text, READY, unread retention, first-seen countdown, dismissal, close, queue promotion and queued cancellation passed.')
}finally{
 assert.equal(path.dirname(path.resolve(testData)),path.resolve(os.tmpdir()))
 assert(path.basename(testData).startsWith('codebeat-notice-data-test-'))
 await rm(testData,{recursive:true,force:true,maxRetries:10,retryDelay:200})
}
