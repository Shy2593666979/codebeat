import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'
test.use({viewport:{width:1200,height:800},locale:'zh-CN',timezoneId:'Asia/Shanghai'})
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const name=new URL(route.request().url()).pathname;await route.fulfill({path:path.resolve('dist',name==='/'?'index.html':name.slice(1))})})
 await installDesktopFixture(page)
})
test('lunch preview and invalid time validation remain usable',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/settings');const card=page.locator('.pretrigger-settings')
 await expect(card.locator('.probe-times b')).toHaveText(['07:30','12:30','17:30'])
 await card.getByRole('switch',{name:'预启动午休'}).click();await expect(card.locator('.probe-times b')).toHaveText(['07:00','12:00','17:00'])
 await card.getByLabel('工作开始时间',{exact:true}).fill('');await expect(card.getByRole('alert')).toBeVisible()
})
test('early manual use replaces old nodes, later use keeps history, and all rows share the axis',async({page})=>{
 await page.clock.setSystemTime(new Date('2026-10-15T22:30:00Z'))
 await page.addInitScript(()=>{
  const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke
  g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{
   if(command!=='get_optimizer_status')return invoke(command,args)
   const later=sessionStorage.getItem('coding-later')==='true'
   return {plans:[{provider:'codex',date:'2026-10-16',status:'active',actual_reset:later?'2026-10-16T09:00:00Z':'2026-10-16T03:20:00Z',recommendations:later?['2026-10-16T09:01:00Z']:['2026-10-16T03:21:00Z','2026-10-16T08:21:00Z']}],windows:[{provider:'codex',start:'2026-10-15T22:20:00Z',end:'2026-10-16T03:20:00Z'},...(later?[{provider:'codex',start:'2026-10-16T04:00:00Z',end:'2026-10-16T09:00:00Z'}]:[])]}
  }
 })
 await page.goto('http://127.0.0.1:1420/#/settings');const card=page.locator('.pretrigger-settings')
 await expect(card.locator('.probe-times b')).toHaveText(['06:20','11:20','16:20'])
 await expect(card.locator('.probe-times>.confirmed b')).toHaveText(['06:20'])
 await expect(card.locator('.probe-activity')).toHaveText('窗口计时中 · 11:20 恢复')
 const tracks=await card.locator('.timeline-track').evaluateAll(elements=>elements.map(el=>({x:el.getBoundingClientRect().x,width:el.getBoundingClientRect().width})))
 expect(tracks[0].x).toBe(tracks[1].x);expect(tracks[0].width).toBe(tracks[1].width)
 await expect(card.locator('.actual-segment').first()).toHaveText('')
 await expect(card.locator('.codex-axis>span')).toHaveText(['06:20','11:20','16:20','21:20'])
 await page.evaluate(()=>sessionStorage.setItem('coding-later','true'));await page.clock.setSystemTime(new Date('2026-10-16T04:30:00Z'));await page.clock.runFor(15100)
 await expect(card.locator('.probe-times b')).toHaveText(['06:20','12:00','17:00'])
 await expect(card.locator('.probe-times>.confirmed b')).toHaveText(['06:20','12:00'])
 await expect(card.locator('.probe-times>.confirmed small')).toHaveText(['5H额度触发','5H额度触发'])
 await expect(card.locator('.probe-activity')).toHaveText('窗口计时中 · 17:00 恢复')
 await page.reload();await expect(card.locator('.actual-segment')).toHaveCount(2)
 await card.screenshot({path:'../docs/screenshots/awo/dynamic-coding.png',animations:'disabled'})
})

test('shared ruler includes lunch, full final window, fixed labels and accumulating execution minutes',async({page})=>{
 await page.clock.setSystemTime(new Date('2026-10-16T04:30:00Z'))
 await page.addInitScript(()=>{
  const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke
  g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{
   if(command==='get_optimizer_status')return {plans:[{provider:'codex',date:'2026-10-16',status:'active',day_start:'2026-10-15T23:10:00Z',display_times:['2026-10-15T23:10:00Z','2026-10-16T04:10:00Z','2026-10-16T09:10:00Z'],slots:[{display_at:'2026-10-16T09:10:00Z',execute_at:'2026-10-16T09:12:00Z'}],actual_reset:'2026-10-16T09:11:00Z'}],windows:[{provider:'codex',source:'app',start:'2026-10-15T23:10:00Z',end:'2026-10-16T04:10:00Z',display_start:'2026-10-15T23:10:00Z'},{provider:'codex',source:'app',start:'2026-10-16T04:11:00Z',end:'2026-10-16T09:11:00Z',display_start:'2026-10-16T04:10:00Z'}]}
   const result=await invoke(command,args);if(command==='get_settings')result.pretrigger.lunch_enabled=true;return result
  }
 })
 await page.goto('http://127.0.0.1:1420/#/settings');const card=page.locator('.pretrigger-settings')
 await expect(card.locator('.probe-times b')).toHaveText(['07:10','12:11','17:11'])
 await expect(card.locator('.probe-times>.confirmed small')).toHaveText(['5H额度触发','5H额度触发'])
 await expect(card.locator('.probe-times>span').last()).toHaveAttribute('title',/17:12/)
 await expect(card.locator('.work-axis>span')).toHaveText(['10:00','12:00','14:00','20:00'])
 await expect(card.locator('.codex-axis>span')).toHaveText(['07:10','12:11','17:11','22:11'])
 for(const width of [1200,620,950]){
  await page.setViewportSize({width,height:800});await page.waitForTimeout(100)
  const tracks=await card.locator('.timeline-track').evaluateAll(es=>es.map(e=>({x:e.getBoundingClientRect().x,w:e.getBoundingClientRect().width})))
  expect(tracks[0]).toEqual(tracks[1])
  for(const axis of ['.work-axis','.codex-axis']){
   const boxes=await card.locator(axis+'>span').evaluateAll(es=>es.map(e=>{const r=e.getBoundingClientRect();return {x:r.x,y:r.y,right:r.right,bottom:r.bottom}}))
   expect(new Set(boxes.map(b=>b.y)).size).toBe(1)
   for(let i=0;i<boxes.length;i++)for(let j=i+1;j<boxes.length;j++){
    const a=boxes[i],b=boxes[j];expect(a.right<=b.x+.5||b.right<=a.x+.5||a.bottom<=b.y+.5||b.bottom<=a.y+.5).toBe(true)
   }
  }
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
 }
 await card.screenshot({path:'../docs/screenshots/awo/shared-ruler-fixed-day.png',animations:'disabled'})
})

test('the latest confirmed window determines the current recovery time',async({page})=>{
 await page.clock.setSystemTime(new Date('2026-10-16T07:10:00Z'))
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>command==='get_optimizer_status'?{plans:[],windows:[{provider:'codex',source:'observed',start:'2026-10-16T06:01:00Z',end:'2026-10-16T11:01:00Z'},{provider:'codex',source:'observed',start:'2026-10-16T07:04:00Z',end:'2026-10-16T12:04:00Z'}]}:invoke(command,args)})
 await page.goto('http://127.0.0.1:1420/#/settings')
 await expect(page.locator('.probe-activity')).toHaveText('窗口计时中 · 20:04 恢复')
})

test('actual request time and official recovery replace stale planned labels',async({page})=>{
 await page.clock.setSystemTime(new Date('2026-10-21T06:00:00Z'))
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>command==='get_optimizer_status'?{plans:[{provider:'codex',date:'2026-10-21',day_start:'2026-10-21T00:18:43Z',slots:[{display_at:'2026-10-21T10:18:43Z',execute_at:'2026-10-21T10:21:39Z'}]}],windows:[{provider:'codex',source:'app',start:'2026-10-21T00:18:43Z',end:'2026-10-21T05:18:43Z'},{provider:'codex',source:'app',start:'2026-10-21T05:20:39Z',end:'2026-10-21T10:20:39Z',display_start:'2026-10-21T05:18:43Z'}]}:invoke(command,args)})
 await page.goto('http://127.0.0.1:1420/#/settings');const card=page.locator('.pretrigger-settings')
 await expect(card.locator('.probe-times b')).toHaveText(['08:18','13:20','18:20'])
 await expect(card.locator('.probe-times small')).toHaveText(['5H额度触发','5H额度触发','预计触发'])
 await expect(card.locator('.probe-activity')).toHaveText('窗口计时中 · 18:20 恢复')
 await expect(card.locator('.probe-times>span').last()).toHaveAttribute('title',/18:21/)
 await expect(card.locator('.node-status em')).toHaveText(['已完成','进行中','待开始'])
 await expect(card.locator('.timeline-row-label')).toHaveCount(0)
 await expect(card.locator('.actual-segment.completed')).toHaveCount(1)
 await expect(card.locator('.actual-segment.active')).toHaveCount(1)
 const geometry=await card.locator('.optimizer-timeline').evaluate(el=>{const track=el.querySelector('.timeline-track')!.getBoundingClientRect(),style=getComputedStyle(el);return {track:track.width,inner:el.clientWidth-parseFloat(style.paddingLeft)-parseFloat(style.paddingRight)}})
 expect(Math.abs(geometry.track-geometry.inner)).toBeLessThan(1)
 await card.screenshot({path:'../docs/screenshots/awo/actual-trigger-times.png',animations:'disabled'})
 await page.setViewportSize({width:620,height:800})
 await expect(card.locator('.node-status em')).toHaveText(['已完成','进行中','待开始'])
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
 await card.screenshot({path:'../docs/screenshots/awo/allocation-states-narrow.png',animations:'disabled'})
 await page.evaluate(()=>document.documentElement.dataset.appearance='dark')
 await card.screenshot({path:'../docs/screenshots/awo/allocation-states-dark.png',animations:'disabled'})
})

test('successful request is active immediately while official timing is still unconfirmed',async({page})=>{
 await page.clock.setSystemTime(new Date('2026-10-21T10:23:20Z'))
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>command==='get_optimizer_status'?{plans:[{provider:'codex',date:'2026-10-21',status:'unknown',slots:[]}],windows:[{provider:'codex',source:'app_unconfirmed',start:'2026-10-21T10:23:10Z',end:'2026-10-21T15:23:10Z'}]}:invoke(command,args)})
 await page.goto('http://127.0.0.1:1420/#/settings');const card=page.locator('.pretrigger-settings')
 await expect(card.locator('.probe-times>.active b')).toHaveText('18:23')
 await expect(card.locator('.probe-times>.active .node-status em')).toHaveText('进行中')
 await expect(card.locator('.probe-times>.active small')).toHaveText('5H额度触发')
 await expect(card.locator('.actual-segment.active')).toHaveCount(1)
 await expect(card.locator('.probe-activity')).toHaveText('已触发，预计 23:23 恢复')
})

test('reset cycles use single-line labels and only their own internal dividers',async({page})=>{
 await page.clock.setSystemTime(new Date('2026-10-20T10:00:00Z'))
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>command==='get_optimizer_status'?{plans:[{provider:'codex',date:'2026-10-20',day_start:'2026-10-20T00:40:00Z',display_times:['2026-10-20T00:40:00Z','2026-10-20T05:40:00Z','2026-10-20T10:40:00Z'],slots:[]}],windows:[
  {provider:'codex',source:'observed',start:'2026-10-20T00:40:00Z',end:'2026-10-20T05:40:00Z'},
  {provider:'codex',source:'observed',start:'2026-10-20T06:01:00Z',end:'2026-10-20T11:01:00Z',closed_at:'2026-10-20T06:43:00Z'},
  {provider:'codex',source:'observed',start:'2026-10-20T07:04:00Z',end:'2026-10-20T12:04:00Z'},
 ]}:invoke(command,args)})
 await page.goto('http://127.0.0.1:1420/#/settings');const card=page.locator('.pretrigger-settings')
 await expect(card.locator('.codex-axis>span')).toHaveText(['08:40','14:01','14:43','15:04','20:04'])
 await expect(card.locator('.work-axis>span')).toHaveText(['10:00','20:00'])
 await expect(card.locator('.codex-axis>.reset')).toHaveAttribute('title','官方重置')
 await expect(card.locator('.actual-segment')).toHaveCount(3)
 await expect(card.locator('.actual-segment').last()).toHaveAttribute('title',/15:04/)
 await expect(card.locator('.work-track .timeline-gridline')).toHaveCount(0)
 await expect(card.locator('.timeline-row').last().locator('.timeline-gridline')).toHaveCount(4)
 const separators=await card.locator('.timeline-gridline').evaluateAll(es=>es.map(e=>({width:getComputedStyle(e).width,background:getComputedStyle(e).backgroundImage})))
 expect(separators.every(s=>s.width==='1px'&&s.background==='none')).toBe(true)
 const labelRows=await card.locator('.codex-axis>span').evaluateAll(es=>es.map(e=>e.getBoundingClientRect().y))
 expect(new Set(labelRows).size).toBe(1)
 await card.screenshot({path:'../docs/screenshots/awo/reset-cycles-dotted-ruler.png',animations:'disabled'})
})
test('work end switches to the next day immediately instead of midnight',async({page})=>{
 await page.clock.setSystemTime(new Date('2026-10-19T11:59:55Z'))
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{const result=await invoke(command,args);if(command==='get_settings')result.pretrigger.days='daily';return result}})
 await page.goto('http://127.0.0.1:1420/#/settings');const card=page.locator('.pretrigger-settings')
 await expect(card.locator('.probe-plan>strong')).toContainText('2026-10-19')
 await page.clock.runFor(6100)
 await expect(card.locator('.probe-plan>strong')).toHaveText('下次工作计划 · 2026-10-20')
 await expect(card.locator('.probe-times>.confirmed')).toHaveCount(0)
})
test('compact English card stays translated and within the settings columns',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{const result=await invoke(command,args);if(command==='get_settings')result.language='en-US';return result}})
 await page.setViewportSize({width:620,height:740});await page.goto('http://127.0.0.1:1420/#/settings')
 await expect(page.locator('.pretrigger-settings').getByRole('heading',{name:'Daily coding quota',exact:true})).toBeVisible()
 await expect(page.locator('.pretrigger-settings')).not.toContainText(/[\u4e00-\u9fff]/)
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
})

test('legacy active status without verified evidence creates no triggered cards',async({page})=>{
 await page.addInitScript(()=>{
  const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke
  g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>command==='get_optimizer_status'?{
   plans:[{provider:'codex',date:'2026-10-16',status:'active',actual_reset:'2026-10-16T05:36:00Z',recommendations:['2026-10-16T05:37:00Z','2026-10-16T10:37:00Z']}],windows:[],
  }:invoke(command,args)
 })
 await page.goto('http://127.0.0.1:1420/#/settings')
 const card=page.locator('.pretrigger-settings')
 await expect(card.locator('.probe-times>.confirmed')).toHaveCount(0)
 await expect(card.locator('.actual-segment')).toHaveCount(0)
 await expect(card.locator('.probe-activity')).not.toContainText('窗口计时中')
 await expect(card.locator('.probe-times b')).toHaveText(['07:30','12:30','17:30'])
 await page.clock.runFor(31000)
 await expect(card.locator('.probe-times>.confirmed')).toHaveCount(0)
 await page.clock.setSystemTime(new Date('2026-10-16T04:05:00Z'));await page.clock.runFor(1100)
 await expect(card.locator('.probe-times>.missed small')).toHaveText(['未触发'])
 await card.screenshot({path:'../docs/screenshots/awo/zero-usage-no-fake-triggers.png',animations:'disabled'})
})
