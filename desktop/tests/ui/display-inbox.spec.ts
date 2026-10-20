import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'
test.use({viewport:{width:1200,height:820},locale:'zh-CN',timezoneId:'Asia/Shanghai'})
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const n=new URL(route.request().url()).pathname;await route.fulfill({path:path.resolve('dist',n==='/'?'index.html':n.slice(1))})})
 await installDesktopFixture(page)
})
const first={id:'d27dfaa2-8399-401d-9428-000000000001',title:'任务已完成',message:'项目构建完成，可以继续工作。',kind:'success',task_id:null,page:'tasks',at:'2026-10-16T06:40:00Z'}
const second={...first,id:'d27dfaa2-8399-401d-9428-000000000002',title:'额度已恢复',message:'Codex 五小时额度窗口已恢复。',kind:'info',page:'usage'}
async function seed(page:import('@playwright/test').Page,tray=false){await page.addInitScript(({notices,tray})=>{const g=window as any;g.__inboxNotices=notices;if(tray)g.__trayLayout={opensLeft:false,tail:294,above:true,messages:true}}, {notices:[first,second],tray})}
test('display scale applies immediately, survives navigation and resize saves normal bounds',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/settings')
 const select=page.getByRole('combobox',{name:'界面缩放',exact:true});await expect(select).toContainText('按屏幕适配')
 await select.click();await page.getByRole('option',{name:'110%',exact:true}).click();await expect(select).toContainText('110%')
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>c.command==='set_display_scale').at(-1).args.value)).toBe(1.1)
 await page.getByRole('link',{name:'首页',exact:true}).click();await page.getByRole('link',{name:'设置',exact:true}).click();await expect(select).toContainText('110%')
 await page.evaluate(()=>(window as any).__codebeatEmit('tauri://resize',{width:1320,height:902}));await page.clock.runFor(650)
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.some((c:any)=>c.command==='save_window_layout'))).toBe(true)
 await page.screenshot({path:'../docs/screenshots/display-inbox/settings.png',animations:'disabled'})
})
test('background messages persist, then focused presentation acknowledges and closes after two seconds',async({page})=>{
 await seed(page)
 await page.addInitScript(()=>{const g=window as any;g.__mainHidden=true;Object.defineProperty(document,'hidden',{configurable:true,get:()=>g.__mainHidden})})
 await page.goto('http://127.0.0.1:1420/#/');await page.evaluate(()=>document.fonts.ready)
 await expect(page.locator('.feedback-card')).toHaveCount(2);await page.clock.runFor(10_000)
 expect(await page.evaluate(()=>(window as any).__inboxNotices.length)).toBe(2)
 await page.evaluate(()=>{(window as any).__mainHidden=false;document.dispatchEvent(new Event('visibilitychange'))})
 await expect.poll(()=>page.evaluate(()=>(window as any).__inboxNotices.length)).toBe(0)
 await expect(page.locator('.feedback-card')).toHaveCount(2);await expect(page.locator('.feedback-countdown')).toHaveCount(2)
 await page.clock.runFor(2100);await expect(page.locator('.feedback-card')).toHaveCount(0)
})
test('hover list preserves messages, opens the selected target and has ignore-all at its bottom',async({page})=>{
 await page.setViewportSize({width:515,height:527});await seed(page,true);await page.goto('http://127.0.0.1:1420/#/tray')
 const menu=page.getByRole('menu',{name:'消息列表',exact:true});await expect(menu).toBeVisible();await expect(menu.locator('.inbox-item')).toHaveCount(2)
 expect(await page.evaluate(()=>(window as any).__inboxNotices.length)).toBe(2)
 await expect(menu).not.toContainText('已读');await expect(menu).not.toContainText('未读')
 const ignore=(await page.getByRole('menuitem',{name:'忽略全部',exact:true}).boundingBox())!,items=(await menu.locator('.inbox-items').boundingBox())!;expect(ignore.y).toBeGreaterThanOrEqual(items.y+items.height)
 await page.evaluate(()=>document.fonts.ready);await page.screenshot({path:'../docs/screenshots/display-inbox/tray-messages.png',omitBackground:true,animations:'disabled'})
 await menu.locator('.inbox-item').last().click();expect(await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>c.command==='open_notice').at(-1).args.id)).toBe(second.id)
 await page.getByRole('menuitem',{name:'忽略全部',exact:true}).click();await expect(menu.locator('.inbox-item')).toHaveCount(0)
 expect(await page.evaluate(()=>(window as any).__inboxNotices.length)).toBe(0)
 await page.evaluate(notice=>{const g=window as any;g.__inboxNotices=[notice];g.__codebeatEmit('notices-changed',[notice])},first)
 await expect(menu.locator('.inbox-item')).toHaveCount(1)
})
test('ignore failure preserves the list and keyboard navigation includes ignore all',async({page})=>{
 await seed(page,true);await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=(command:string,args:any)=>command==='ignore_all_notices'?Promise.reject(Error('ignore failed fixture')):invoke(command,args)})
 await page.goto('http://127.0.0.1:1420/#/tray');await expect(page.locator('.inbox-item')).toHaveCount(2)
 await page.locator('.inbox-item').first().focus();await page.keyboard.press('End');await expect(page.getByRole('menuitem',{name:'忽略全部',exact:true})).toBeFocused()
 await page.keyboard.press('Enter');await expect(page.getByRole('alert')).toContainText('ignore failed fixture');await expect(page.locator('.inbox-item')).toHaveCount(2)
})

test('native hidden window cannot acknowledge even when WebView reports visible and focused',async({page})=>{
 await seed(page)
 await page.addInitScript(()=>{const g=window as any;g.__nativeCanPresent=false;const invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=(command:string,args:any)=>command==='can_present_notices'?Promise.resolve(g.__nativeCanPresent):invoke(command,args)})
 await page.goto('http://127.0.0.1:1420/#/');await page.evaluate(()=>document.fonts.ready)
 await expect(page.locator('.feedback-card')).toHaveCount(2);await page.clock.runFor(5000)
 await page.locator('.feedback-card').first().hover();await page.clock.runFor(3000)
 expect(await page.evaluate(()=>(window as any).__inboxNotices.length)).toBe(2)
 await expect(page.locator('.feedback-countdown')).toHaveCount(0)
 await page.evaluate(()=>{(window as any).__nativeCanPresent=true;window.dispatchEvent(new Event('focus'))})
 await expect.poll(()=>page.evaluate(()=>(window as any).__inboxNotices.length)).toBe(0)
 await page.clock.runFor(2100);await expect(page.locator('.feedback-card')).toHaveCount(0)
})

test('inbox has a bubble heading, no pointer or scroll frame and at most two body lines',async({page})=>{
 await page.addInitScript(notice=>{const g=window as any;g.__inboxNotices=Array.from({length:2},(_,i)=>({...notice,id:'d27dfaa2-8399-401d-9428-'+String(i).padStart(12,'0'),title:i===0?'演示消息 · Codex 自动继续':'演示消息 · Codex 额度提醒',message:'这是一条较长的任务消息。'.repeat(12),kind:['success','info'][i%2]}));g.__trayLayout={opensLeft:false,tail:0,above:true,messages:true,scale:.85}},first)
 await page.goto('http://127.0.0.1:1420/#/tray')
 for(const scale of [.68,.85,1.02]){
  await page.setViewportSize({width:Math.ceil(354*scale),height:Math.ceil(338*scale)})
  await page.evaluate(scale=>{const g=window as any;g.__trayLayout={opensLeft:false,tail:0,above:true,messages:true,scale};g.__codebeatEmit('tray-open',g.__trayLayout)},scale)
  const menu=page.getByRole('menu',{name:'消息列表',exact:true})
  await expect(menu.locator('.inbox-count')).toHaveText('2')
  await expect(menu.locator('.inbox-item')).toHaveCount(2)
  await expect(menu.locator('.inbox-heading-icon svg')).toHaveCount(1)
  await expect(menu.locator('.inbox-heading .brand-mark')).toHaveCount(0)
  await expect(menu.locator('time').first()).toHaveText('14:40')
  expect(await menu.evaluate(el=>getComputedStyle(el,':after').display)).toBe('none')
  expect(await menu.locator('.inbox-items').evaluate(el=>el.scrollHeight===el.clientHeight&&getComputedStyle(el).overflowY==='visible')).toBe(true)
  const bounds=(await menu.boundingBox())!,footer=(await menu.locator('footer').boundingBox())!
  expect(bounds.y).toBeGreaterThanOrEqual(0)
  expect(bounds.x).toBeCloseTo(0,1)
  expect(footer.y+footer.height).toBeLessThanOrEqual(Math.ceil(338*scale)+1)
  await expect(menu.locator('.inbox-item').last()).toBeInViewport()
  for(const body of await menu.locator('.inbox-item p').all()){
   expect(await body.evaluate(el=>getComputedStyle(el).webkitLineClamp)).toBe('2')
   expect((await body.boundingBox())!.height).toBeLessThanOrEqual(40*scale+1)
   expect(await body.evaluate(el=>el.scrollHeight>el.clientHeight)).toBe(true)
  }
  await page.evaluate(()=>document.fonts.ready)
  await page.screenshot({path:`../docs/screenshots/display-inbox/tray-refined-${scale}.png`,omitBackground:true,animations:'disabled'})
 }
 await page.getByRole('button',{name:'关闭',exact:true}).click()
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.some((c:any)=>c.command==='tray_action'&&c.args.action==='hide'))).toBe(true)
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.some((c:any)=>c.command==='acknowledge_notices'))).toBe(false)
})

test('all notices remain in the full-height list without an internal scrollbar',async({page})=>{
 const scale=.5,count=8
 await page.setViewportSize({width:Math.ceil(354*scale),height:Math.ceil((110+114*count)*scale)})
 await page.addInitScript(({notice,scale,count})=>{const g=window as any;g.__inboxNotices=Array.from({length:count},(_,i)=>({...notice,id:'d27dfaa2-8399-401d-9428-'+String(i).padStart(12,'0')}));g.__trayLayout={opensLeft:false,tail:0,above:true,messages:true,scale}}, {notice:first,scale,count})
 await page.goto('http://127.0.0.1:1420/#/tray')
 const menu=page.getByRole('menu',{name:'消息列表',exact:true})
 await expect(menu.locator('.inbox-item')).toHaveCount(count)
 await expect(menu.locator('.inbox-item').first()).toBeInViewport()
 await expect(menu.locator('.inbox-item').last()).toBeInViewport()
 expect(await menu.locator('.inbox-items').evaluate(el=>el.scrollHeight===el.clientHeight)).toBe(true)
})
