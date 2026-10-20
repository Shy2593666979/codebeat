// 测试真实 WebView2 通知进程；使用隔离配置和确认文件，不发送模型请求。
import {spawn} from 'node:child_process'
import {mkdtemp,mkdir,writeFile,rm,stat} from 'node:fs/promises'
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
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms))
async function port(){const server=net.createServer();await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const result=server.address().port;await new Promise(resolve=>server.close(resolve));return result}
async function exists(file){try{await stat(file);return true}catch{return false}}
async function helper(kind='success',page='tasks',queued=false){
 const id=randomUUID(),profile=await mkdtemp(path.join(os.tmpdir(),'codebeat-notice-test-')),debugPort=await port()
 const ack=path.join(ackDirectory,id+'.read'),dismiss=path.join(ackDirectory,id+'.dismiss')
 const started=Date.now(),child=spawn(executable,['--notification-helper'],{windowsHide:true,env:{...process.env,LOCALAPPDATA:testData,WEBVIEW2_USER_DATA_FOLDER:profile,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${debugPort} --remote-debugging-address=127.0.0.1`},stdio:['pipe','pipe','pipe']})
 let stdout='',stderr='';child.stdout.on('data',b=>stdout+=b);child.stderr.on('data',b=>stderr+=b)
 child.stdin.end(JSON.stringify({title:'CodeBeat 本地通知测试',body:'<script>&" 中文 $(literal)',kind,page,task_id:null,sound:false,ack_path:ack,dismiss_path:dismiss}))
 let browser
 async function cleanup(){
  child.kill();if(browser)await browser.close();await delay(250)
  for(const file of [ack,dismiss])await rm(file,{force:true})
  assert.equal(path.dirname(path.resolve(profile)),path.resolve(os.tmpdir()))
  assert(path.basename(profile).startsWith('codebeat-notice-test-'))
  await rm(profile,{recursive:true,force:true,maxRetries:10,retryDelay:200}).catch(()=>{})
 }
 try{
  const deadline=Date.now()+20000
  if(queued){
   while(!stdout.includes('READY')&&Date.now()<deadline&&child.exitCode===null)await delay(50)
   assert(stdout.includes('READY'),`Queued helper never reported READY: ${stderr}`)
   assert(Date.now()-started<8000,'Queued helper exceeded runner readiness timeout')
   return {child,ack,dismiss,cleanup,async rendered(){
    while(!browser&&Date.now()<deadline){try{browser=await chromium.connectOverCDP(`http://127.0.0.1:${debugPort}`)}catch{await delay(100)}}
    assert(browser,'Queued helper never constructed its WebView after slot release')
    let view
    while(!view&&Date.now()<deadline){view=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('#/notification'));if(!view)await delay(100)}
    assert(view,'Queued helper never acquired a window');await view.locator('.native-notification').waitFor();return view
   }}
  }
  while(!browser&&Date.now()<deadline){
   if(child.exitCode!==null)throw Error(`Helper exited ${child.exitCode}: ${stderr}`)
   try{const response=await fetch(`http://127.0.0.1:${debugPort}/json/version`);if(response.ok)browser=await chromium.connectOverCDP(`http://127.0.0.1:${debugPort}`)}catch{}
   if(!browser)await delay(100)
  }
  assert(browser,`No WebView2 debugging endpoint: ${stderr}`)
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
  while(!stdout.includes('READY')&&Date.now()<deadline)await delay(50)
  assert(stdout.includes('READY'),`Helper never reported READY: ${stderr}`)
  assert(Date.now()-started<8000,'Helper exceeded runner readiness timeout')
  return {child,view,ack,dismiss,profile,browser,cleanup}
 }catch(error){await cleanup();throw error}
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
