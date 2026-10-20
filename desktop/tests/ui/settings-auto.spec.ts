import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const n=new URL(route.request().url()).pathname,file=path.resolve('dist',n==='/'?'index.html':n.slice(1));if(!file.startsWith(path.resolve('dist')+path.sep)){await route.abort();return}await route.fulfill({path:file})})
 await installDesktopFixture(page)
})
test('default window fits all common settings on one page',async({page})=>{
 await page.setViewportSize({width:1200,height:820})
 await page.goto('http://127.0.0.1:1420/#/settings');await page.evaluate(()=>document.fonts.ready)
 await expect(page.locator('.probe-times b')).toHaveText(['07:30','12:30','17:30'])
 await page.screenshot({path:'../docs/screenshots/settings-auto/default.png',animations:'disabled'})
 expect(await page.locator('.workspace').evaluate(el=>el.scrollHeight<=el.clientHeight)).toBe(true)
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
 await expect(page.locator('.settings-savebar')).toHaveCount(0)
 await expect(page.locator('.settings-more')).toHaveCount(0)
 await expect(page.getByText('更改自动生效',{exact:true})).toHaveCount(0)
 await page.screenshot({path:'../docs/screenshots/settings-auto/default.png',animations:'disabled'})
 await page.getByRole('combobox',{name:'系统主题',exact:true}).click();await page.getByRole('option',{name:'深色模式',exact:true}).click();await page.clock.runFor(300)
 await expect(page.locator('html')).toHaveAttribute('data-appearance','dark')
 await page.screenshot({path:'../docs/screenshots/settings-auto/default-dark.png',animations:'disabled'})
 await expect(page.locator('.feedback-card')).toHaveCount(0)
})
test('invalid times stay local and rapid edits flush when navigating',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/settings')
 const end=page.getByLabel('工作结束时间');await end.fill('10:00');await page.clock.runFor(500)
 await expect(page.locator('.settings-auto-state')).toContainText('上下班时间')
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>c.command==='update_settings').length)).toBe(0)
 await end.fill('21:00')
 await page.getByRole('switch',{name:'播放提示音'}).click()
 await page.locator('.sidebar nav a[href="#/"]').click()
 await page.locator('.sidebar nav a[href="#/settings"]').click()
 await expect(end).toHaveValue('21:00');await expect(page.getByRole('switch',{name:'播放提示音'})).toHaveAttribute('aria-checked','false')
 await expect(page.locator('.settings-auto-state')).toHaveCount(0)
 await expect(page.locator('.feedback-card')).toHaveCount(0)
})
test('a failed write shows an inline retry and never a popup',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;let failed=false;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{if(command==='update_settings'&&!failed){failed=true;throw Error('offline fixture')}return invoke(command,args)}})
 await page.goto('http://127.0.0.1:1420/#/settings')
 await page.getByRole('switch',{name:'播放提示音'}).click();await page.clock.runFor(300)
 await expect(page.locator('.settings-auto-state')).toContainText('offline fixture');await expect(page.locator('.feedback-card')).toHaveCount(0)
 await page.getByRole('button',{name:'重试',exact:true}).click()
 await expect(page.locator('.settings-auto-state')).toHaveCount(0)
 await page.locator('.sidebar nav a[href="#/"]').click();await page.locator('.sidebar nav a[href="#/settings"]').click()
 await expect(page.getByRole('switch',{name:'播放提示音'})).toHaveAttribute('aria-checked','false')
})
test('provider glyphs fit their local frames at every settings width',async({page})=>{
 for(const width of [1200,1024,620]){
  await page.setViewportSize({width,height:820})
  await page.goto('http://127.0.0.1:1420/#/settings')
  await expect(page.locator('.provider-settings-row .provider-mark>img')).toHaveCount(2)
  await page.locator('.provider-mark>img').evaluateAll(images=>Promise.all(images.map(img=>(img as HTMLImageElement).decode())))
  const sizes=await page.locator('.provider-mark>img').evaluateAll(images=>images.map(img=>{const image=img.getBoundingClientRect(),frame=img.parentElement!.getBoundingClientRect();return {x:image.x,y:image.y,w:image.width,h:image.height,fx:frame.x,fy:frame.y,fw:frame.width,fh:frame.height}}))
  expect(sizes).toHaveLength(4)
  for(const s of sizes){
   expect(s.w).toBeGreaterThan(s.fw*.5);expect(s.h).toBeGreaterThan(s.fh*.5)
   expect(s.x).toBeGreaterThan(s.fx);expect(s.y).toBeGreaterThan(s.fy)
   expect(s.x+s.w).toBeLessThan(s.fx+s.fw);expect(s.y+s.h).toBeLessThan(s.fy+s.fh)
   expect(Math.abs(s.w-s.h)).toBeLessThan(1)
  }
  expect(sizes[0]!.w).toBeGreaterThan(sizes[2]!.w)
  if(width===1200){await page.evaluate(()=>document.fonts.ready);await page.screenshot({path:'../docs/screenshots/settings-provider-icons.png',animations:'disabled'})}
 }
 await page.goto('http://127.0.0.1:1420/#/')
 const glyph=page.locator('.usage-card .provider-mark>img').first()
 const home=await glyph.evaluate(img=>({image:img.getBoundingClientRect().width,frame:img.parentElement!.getBoundingClientRect().width}))
 expect(home.image).toBeLessThan(home.frame);expect(home.image).toBeGreaterThan(home.frame*.5)
})
