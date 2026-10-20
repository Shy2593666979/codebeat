import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'

test.use({viewport:{width:515,height:527},locale:'zh-CN',timezoneId:'Asia/Shanghai'})
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{
  const name=new URL(route.request().url()).pathname,file=path.resolve('dist',name==='/'?'index.html':name.slice(1))
  if(!file.startsWith(path.resolve('dist')+path.sep)){await route.abort();return}
  await route.fulfill({path:file})
 })
 await installDesktopFixture(page)
})

test('tray reference layout has no shortcuts, real status and provider flyout',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/tray')
 const menu=page.getByRole('menu',{name:'CodeBeat 托盘菜单'})
 await expect(menu).toContainText('运行中 · 预计 1 小时 20 分钟后继续')
 await expect(page.locator('.desktop-shell')).toHaveCount(0)
 await expect(menu).not.toContainText(/Ctrl|Shift|Alt/)
 await expect(menu.getByRole('menuitem')).toHaveCount(8)
 await page.evaluate(()=>document.fonts.ready)
 await expect(page.locator('.tray-submenu')).toHaveCount(0)
 await menu.getByRole('menuitem',{name:'查看运行状态',exact:true}).hover()
 const flyout=page.getByRole('menu',{name:'工具运行与额度'})
 await expect(flyout).toContainText('额度将在 1 小时 20 分钟后恢复')
 await expect(flyout).toContainText('Claude Code')
 await expect(flyout.locator('img')).toHaveCount(2)
 const providerIcons=await flyout.locator('.provider-mark').all()
 expect((await providerIcons[0]!.boundingBox())!.x).toBeCloseTo((await providerIcons[1]!.boundingBox())!.x,1)
 await flyout.getByRole('menuitem').first().hover()
 expect(await flyout.evaluate(el=>getComputedStyle(el,':before').left)).toBe('-7px')
 await page.screenshot({path:'../docs/screenshots/tray/status.png',omitBackground:true,animations:'disabled'})
 const card=(await menu.boundingBox())!,sub=(await flyout.boundingBox())!
 expect(card.width).toBeCloseTo(277.1,1);expect(sub.x).toBeGreaterThan(card.x+card.width)
 expect(card.y+card.height).toBeLessThan(527)
 expect(await page.evaluate(()=>getComputedStyle(document.body).backgroundColor)).toBe('rgba(0, 0, 0, 0)')
 await flyout.getByRole('menuitem').first().click()
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.some((c:any)=>c.command==='tray_action'&&c.args.action==='usage'))).toBe(true)
 await page.evaluate(()=>(window as any).__codebeatEmit('tray-open',{opensLeft:true,tail:290,above:false}))
 await menu.getByRole('menuitem',{name:'额度概览',exact:true}).hover()
 const reversed=(await page.getByRole('menu',{name:'工具运行与额度'}).boundingBox())!
 expect(reversed.x).toBeLessThan((await menu.boundingBox())!.x)
 expect(await page.getByRole('menu',{name:'工具运行与额度'}).evaluate(el=>getComputedStyle(el,':before').right)).toBe('-7px')
 await page.screenshot({path:'../docs/screenshots/tray/edge-flipped.png',omitBackground:true,animations:'disabled'})
})

test('tray keyboard, task links, pause and immediate continuation use explicit IPC actions',async({page})=>{
 await page.addInitScript(()=>{
  const globals=window as any,invoke=globals.__TAURI_INTERNALS__.invoke
  globals.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{
   const value=await invoke(command,args)
   if(command==='get_tray_snapshot'){value.settings.authorized_roots=[];value.tasks[0].request.consent=false}
   return value
  }
 })
 await page.goto('http://127.0.0.1:1420/#/tray')
 const menu=page.getByRole('menu',{name:'CodeBeat 托盘菜单'})
 await expect(menu.getByRole('menuitem',{name:'立即继续',exact:true})).toBeEnabled()
 await menu.getByRole('menuitem',{name:'暂停所有任务'}).click()
 await expect(menu).toContainText('所有任务已暂停')
 await expect(menu.getByRole('menuitem',{name:'立即继续',exact:true})).toBeDisabled()
 await menu.getByRole('menuitem',{name:'恢复所有任务'}).click()
 await expect(menu.getByRole('menuitem',{name:'立即继续',exact:true})).toBeEnabled()
 await menu.getByRole('menuitem',{name:'立即继续',exact:true}).click()
 const calls=await page.evaluate(()=>(window as any).__codebeatCalls)
 expect(calls.find((c:any)=>c.command==='tray_action'&&c.args.action==='continue').args.taskId).toBe('b27dfaa2-8399-401d-9428-000000000000')
 const tasks=menu.getByRole('menuitem',{name:'任务列表',exact:true})
 await tasks.focus();await page.keyboard.press('ArrowRight')
 const flyout=page.getByRole('menu',{name:'任务列表',exact:true})
 await expect(flyout.getByRole('menuitem').first()).toBeFocused()
 await page.keyboard.press('ArrowDown');await expect(flyout.getByRole('menuitem').nth(1)).toBeFocused()
 await page.screenshot({path:'../docs/screenshots/tray/tasks.png',omitBackground:true,animations:'disabled'})
 await flyout.getByRole('menuitem').first().click()
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.some((c:any)=>c.command==='tray_action'&&c.args.action==='tasks'&&c.args.taskId))).toBeTruthy()
 await page.keyboard.press('Escape');await expect(flyout).toHaveCount(0)
 await page.keyboard.press('Escape')
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.some((c:any)=>c.command==='tray_action'&&c.args.action==='hide'))).toBe(true)
 for(const [name,action] of [['打开主界面','open'],['退出','exit']]){
  await menu.getByRole('menuitem',{name,exact:true}).click()
  expect(await page.evaluate(action=>(window as any).__codebeatCalls.some((c:any)=>c.command==='tray_action'&&c.args.action===action),action)).toBe(true)
 }
})

test('tray errors remain visible without inventing successful execution',async({page})=>{
 await page.addInitScript(()=>{const globals=window as any,invoke=globals.__TAURI_INTERNALS__.invoke;globals.__TAURI_INTERNALS__.invoke=(command:string,args:any)=>command==='tray_action'&&args.action==='continue'?Promise.reject('SCHEDULING_PAUSED'):invoke(command,args)})
 await page.goto('http://127.0.0.1:1420/#/tray')
 await page.getByRole('menuitem',{name:'立即继续',exact:true}).click()
 await expect(page.getByRole('alert')).toHaveText('SCHEDULING_PAUSED')
 await expect(page.getByRole('menu',{name:'CodeBeat 托盘菜单'})).toBeVisible()
})

test('all tray columns align with short, long and wrapped provider or task descriptions',async({page})=>{
 await page.addInitScript(()=>{
  const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke
  g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{
   const value=await invoke(command,args)
   if(command==='get_tray_snapshot'){
    value.usage[0].windows[0].resets_at_utc=new Date(Date.now()+6*86400000+8*3600000).toISOString()
    value.usage[1].windows=[]
    value.tasks.forEach((task:any,i:number)=>task.request.title=i%2?'VeryLongUnbrokenTaskTitle'.repeat(4):'短任务')
   }
   return value
  }
 })
 for(const [scale,opensLeft] of [[.68,false],[.85,true],[1.02,true]] as const){
  await page.setViewportSize({width:Math.round(606*scale),height:Math.round(620*scale)})
  await page.mouse.move(0,0);await page.goto('http://127.0.0.1:1420/#/tray')
  await page.evaluate(layout=>{const g=window as any;g.__trayLayout=layout;g.__codebeatEmit('tray-open',layout)},{scale,opensLeft,above:true,tail:290})
  const menu=page.getByRole('menu',{name:'CodeBeat 托盘菜单',exact:true})
  for(const name of ['查看运行状态','额度概览','任务列表']){
   await menu.getByRole('menuitem',{name,exact:true}).hover()
   const rows=page.locator('.tray-submenu .tray-provider,.tray-submenu .tray-task')
   await expect(rows.first()).toBeVisible()
   const boxes=await rows.evaluateAll(rows=>rows.map(row=>{
    const icon=row.querySelector('.provider-mark')!.getBoundingClientRect(),title=row.querySelector('strong')!.getBoundingClientRect(),copy=row.querySelector('small')!.getBoundingClientRect()
    return {iconX:icon.x,iconWidth:icon.width,titleX:title.x,copyX:copy.x,overflow:row.scrollWidth>row.clientWidth}
   }))
   expect(boxes.length).toBeGreaterThan(1)
   for(const box of boxes){expect(box.iconX).toBeCloseTo(boxes[0]!.iconX,1);expect(box.titleX).toBeCloseTo(boxes[0]!.titleX,1);expect(box.copyX).toBeCloseTo(box.titleX,1);expect(box.iconWidth).toBeCloseTo(32*scale,0);expect(box.overflow).toBe(false)}
   await page.screenshot({path:`../docs/screenshots/tray/aligned-${name==='查看运行状态'?'status':name==='额度概览'?'usage':'tasks'}-${scale}.png`,omitBackground:true,animations:'disabled'})
  }
 }
})

test('flyout pointers align with hovered rows in both directions and monitor scales',async({page})=>{
 for(const [scale,opensLeft,above] of [[.85,false,true],[.68,true,true],[1.02,true,false],[.68,false,false]] as const){
  await page.setViewportSize({width:Math.round(606*scale),height:Math.round(620*scale)})
  await page.mouse.move(0,0)
  await page.goto('http://127.0.0.1:1420/#/tray');await page.evaluate(()=>document.fonts.ready)
  await page.evaluate(layout=>{const g=window as any;g.__trayLayout=layout;g.__codebeatEmit('tray-open',layout)}, {scale,opensLeft,above,tail:290})
  await expect(page.locator('.tray-submenu')).toHaveCount(0)
  const menu=page.getByRole('menu',{name:'CodeBeat 托盘菜单',exact:true})
  expect((await menu.boundingBox())!.width).toBeCloseTo(326*scale,0)
  for(const name of ['查看运行状态','任务列表','额度概览','设置']){
   const row=menu.getByRole('menuitem',{name,exact:true});await row.hover();await expect(page.locator('.tray-submenu')).toBeVisible()
   await expect.poll(()=>page.locator('.tray-submenu').evaluate(el=>Number.parseFloat((el as HTMLElement).style.getPropertyValue('--submenu-tail')))).toBeGreaterThan(0)
   const metrics=await page.locator('.tray-submenu').evaluate((el,scale)=>{
    const sub=el.getBoundingClientRect(),stage=document.querySelector('.tray-stage')!.getBoundingClientRect(),menu=document.querySelector('.tray-menu')!.getBoundingClientRect(),pointer=getComputedStyle(el,':before')
    return {pointer:sub.top+Number.parseFloat(pointer.top)*scale+6.5*scale,top:sub.top,bottom:sub.bottom,stageBottom:stage.bottom,gap:sub.left>menu.left?sub.left-menu.right:menu.left-sub.right}
   },scale)
   const target=(await row.boundingBox())!
   expect(Math.abs(metrics.pointer-(target.y+target.height/2))).toBeLessThan(2)
   expect(metrics.gap).toBeCloseTo(14*scale,0)
   expect(metrics.top).toBeGreaterThanOrEqual(0);expect(metrics.bottom).toBeLessThanOrEqual(metrics.stageBottom)
  }
  await menu.getByRole('menuitem',{name:'查看运行状态',exact:true}).hover()
  await page.screenshot({path:`../docs/screenshots/tray/aligned-${scale}-${opensLeft}-${above}.png`,omitBackground:true,animations:'disabled'})
  await page.evaluate(layout=>{const g=window as any;g.__trayLayout=layout;g.__codebeatEmit('tray-open',layout)}, {scale,opensLeft,above,tail:290})
  await expect(page.locator('.tray-submenu')).toHaveCount(0)
 }
})
