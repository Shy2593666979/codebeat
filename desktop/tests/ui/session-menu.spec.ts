import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'

test.use({viewport:{width:1200,height:820}})
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const n=new URL(route.request().url()).pathname,file=path.resolve('dist',n==='/'?'index.html':n.slice(1));if(!file.startsWith(path.resolve('dist')+path.sep)){await route.abort();return}await route.fulfill({path:file})})
 await installDesktopFixture(page)
})

test('home status and token cards align with their providers',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/')
 for(const width of [1348,1200,1100,980,620]){
  await page.setViewportSize({width,height:820})
  const lower=page.locator('.home-bottom>section'),upper=page.locator('.usage-grid>.usage-card')
  for(let i=0;i<2;i++){const a=(await upper.nth(i).boundingBox())!,b=(await lower.nth(i).boundingBox())!;expect(b.x).toBeCloseTo(a.x,0);expect(b.x+b.width).toBeCloseTo(a.x+a.width,0)}
  if(width===1200)await page.screenshot({path:'../docs/screenshots/session-menu/home-aligned.png',animations:'disabled'})
 }
})

test('sessions have no status tabs or persistent details; menu opens the clicked conversation',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/sessions')
 await expect(page.locator('.session-toolbar .filter-tabs')).toHaveCount(0)
 await expect(page.locator('.sessions-layout>aside')).toHaveCount(0)
 await expect(page.getByRole('dialog')).toHaveCount(0)
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>c.command==='get_conversation_detail').length)).toBe(0)
 const rows=page.locator('.sessions-table tbody tr');await rows.nth(1).click()
 await expect(page.getByRole('dialog')).toHaveCount(0)
 await rows.nth(1).click({button:'right'})
 const menu=page.getByRole('menu',{name:'会话操作'})
 await expect(menu.getByRole('menuitem')).toHaveText(['创建定时任务','立即继续','会话详情'])
 await page.screenshot({path:'../docs/screenshots/session-menu/context-menu.png',animations:'disabled'})
 await menu.getByRole('menuitem',{name:'会话详情'}).click()
 const detail=page.getByRole('dialog',{name:'会话详情',exact:true})
 await expect(detail).toContainText('重构前端组件结构');await expect(detail).toContainText('Claude Code')
 await detail.getByRole('button',{name:'对话内容',exact:true}).click();await expect(detail.locator('.conversation-content')).toContainText('JWT')
 await page.screenshot({path:'../docs/screenshots/session-menu/details.png',animations:'disabled'})
 await page.keyboard.press('Escape');await expect(detail).toHaveCount(0);await expect(rows.nth(1)).toBeFocused()
 await rows.nth(1).click({button:'right'});await menu.getByRole('menuitem',{name:'创建定时任务'}).click()
 const editor=page.getByRole('dialog',{name:'新建任务',exact:true})
 await expect(editor.getByLabel('任务名称')).toHaveValue('继续「重构前端组件结构」')
 await expect(editor.getByRole('button',{name:'Claude Code',exact:true})).toHaveAttribute('aria-pressed','true')
 await expect(page.getByRole('dialog')).toHaveCount(1)
})

test('menu supports keyboard, outside dismissal and viewport clamping',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/sessions')
 const row=page.locator('.sessions-table tbody tr').first(),menu=page.getByRole('menu')
 await row.focus();await page.keyboard.press('Shift+F10')
 await expect(menu.getByRole('menuitem',{name:'创建定时任务'})).toBeFocused()
 await page.keyboard.press('End');await expect(menu.getByRole('menuitem',{name:'会话详情'})).toBeFocused()
 await page.keyboard.press('ArrowDown');await expect(menu.getByRole('menuitem',{name:'创建定时任务'})).toBeFocused()
 await page.keyboard.press('Escape');await expect(menu).toHaveCount(0);await expect(row).toBeFocused()
 await row.click({button:'right'});await page.getByRole('heading',{name:'会话记录',exact:true}).click();await expect(menu).toHaveCount(0)
 // Wait for the actual resize event before opening a new menu; a queued resize
 // legitimately dismisses menus and otherwise races the synthetic contextmenu.
 await page.evaluate(()=>{(window as any).__sessionResizeDone=new Promise(resolve=>window.addEventListener('resize',()=>resolve(null),{once:true}))})
 await page.setViewportSize({width:620,height:640})
 await page.evaluate(()=>(window as any).__sessionResizeDone)
 await page.evaluate(()=>{document.querySelector('.sessions-table tbody tr')!.dispatchEvent(new MouseEvent('contextmenu',{bubbles:true,clientX:innerWidth-2,clientY:innerHeight-2}))})
 const bounds=(await menu.boundingBox())!;expect(bounds.x+bounds.width).toBeLessThanOrEqual(612);expect(bounds.y+bounds.height).toBeLessThanOrEqual(632)
 await page.evaluate(()=>document.documentElement.dataset.appearance='dark')
 await page.screenshot({path:'../docs/screenshots/session-menu/small-dark-menu.png',animations:'disabled'})
 await menu.getByRole('menuitem',{name:'会话详情'}).click()
 const detail=page.getByRole('dialog',{name:'会话详情',exact:true})
 expect(await detail.evaluate(el=>el.scrollWidth<=el.clientWidth)).toBe(true)
 await page.keyboard.press('Shift+Tab');await expect(detail.getByRole('button',{name:'创建定时任务'})).toBeFocused()
 await page.screenshot({path:'../docs/screenshots/session-menu/small-dark-detail.png',animations:'disabled'})
})

test('unrecoverable conversations still expose details while execution stays disabled',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{const result=await invoke(command,args);return command==='list_conversations'?{...result,data:result.data.map((c:any)=>({...c,resumable:false}))}:result}})
 await page.goto('http://127.0.0.1:1420/#/sessions');await page.locator('.sessions-table tbody tr').first().click({button:'right'})
 const menu=page.getByRole('menu');await expect(menu.getByRole('menuitem',{name:'创建定时任务'})).toBeDisabled();await expect(menu.getByRole('menuitem',{name:'立即继续'})).toBeDisabled()
 await expect(menu.getByRole('menuitem',{name:'会话详情'})).toBeFocused()
 await page.keyboard.press('Enter');await expect(page.getByRole('dialog',{name:'会话详情',exact:true})).toBeVisible()
})

test('late details from a closed conversation cannot replace a newly opened one',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;let first=true;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{
  const result=await invoke(command,args)
  if(command==='get_conversation_detail'&&first){first=false;return new Promise(resolve=>{g.__resolveOldDetail=()=>resolve({...result,messages:[{role:'assistant',text:'过期会话内容'}]})})}
  return result
 }})
 await page.goto('http://127.0.0.1:1420/#/sessions')
 const rows=page.locator('.sessions-table tbody tr'),details=page.getByRole('dialog',{name:'会话详情',exact:true})
 await rows.first().click({button:'right'});await page.getByRole('menuitem',{name:'会话详情'}).click()
 await expect.poll(()=>page.evaluate(()=>typeof(window as any).__resolveOldDetail)).toBe('function')
 await page.keyboard.press('Escape');await rows.nth(1).click({button:'right'});await page.getByRole('menuitem',{name:'会话详情'}).click()
 await details.getByRole('button',{name:'对话内容',exact:true}).click();await expect(details.locator('.conversation-content')).toContainText('JWT')
 await page.evaluate(()=>(window as any).__resolveOldDetail());await expect(details).not.toContainText('过期会话内容')
 await details.getByRole('button',{name:'概览',exact:true}).click()
 await page.screenshot({path:'../docs/screenshots/session-menu/details-overview.png',animations:'disabled'})
})
