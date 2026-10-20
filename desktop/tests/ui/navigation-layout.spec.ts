import {test,expect,type Page} from '@playwright/test'
import path from 'node:path'
import {readFileSync} from 'node:fs'
import {installDesktopFixture} from './fixtures'

test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{
  const name=new URL(route.request().url()).pathname,file=path.resolve('dist',name==='/'?'index.html':name.slice(1))
  if(!file.startsWith(path.resolve('dist')+path.sep)){await route.abort();return}
  await route.fulfill({path:file})
 })
 await installDesktopFixture(page)
})

async function geometry(page:Page){
 return page.evaluate(()=>{
  const bounds=(selector:string)=>{const r=document.querySelector(selector)!.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height}}
  const main=document.querySelector('.workspace main')!,style=getComputedStyle(main),r=main.getBoundingClientRect()
  return {sidebar:bounds('.sidebar'),titlebar:bounds('.titlebar'),nav:bounds('.sidebar nav'),workspace:bounds('.workspace'),content:{x:r.x+parseFloat(style.paddingLeft),y:r.y+parseFloat(style.paddingTop),width:main.clientWidth},bodyScroll:window.scrollY}
 })
}

test('default desktop size fits the simplified home and preserves task management',async({page})=>{
 const config=JSON.parse(readFileSync('src-tauri/tauri.conf.json','utf8')).app.windows[0]
 await page.setViewportSize({width:config.width,height:config.height})
 await page.goto('http://127.0.0.1:1420/#/')
 await expect(page.locator('.usage-card')).toHaveCount(2)
 await page.evaluate(()=>document.fonts.ready)
 await expect(page.locator('.home-tasks')).toHaveCount(0)
 const quota=(await page.locator('.usage-grid').boundingBox())!,status=(await page.locator('.today-panel').boundingBox())!,trend=(await page.locator('.home-bottom .token-trend').boundingBox())!
 expect(status.y-quota.y-quota.height).toBeCloseTo(20,0)
 expect(status.y).toBe(trend.y)
 expect(status.height).toBeGreaterThanOrEqual(300)
 expect(status.height).toBeCloseTo(trend.height,0)
 expect(status.y+status.height).toBeLessThan(config.height-15)
 expect(await page.locator('.workspace').evaluate(el=>el.scrollHeight<=el.clientHeight)).toBe(true)
 await page.screenshot({path:'../docs/screenshots/home-window/default.png',animations:'disabled'})
 await page.getByRole('button',{name:'新建任务',exact:true}).click()
 await expect(page.getByRole('dialog',{name:'新建任务',exact:true})).toBeVisible()
 await page.getByRole('button',{name:'关闭',exact:true}).click()
 await page.locator('.sidebar nav a[href="#/tasks"]').click()
 await expect(page.locator('.task-row')).toHaveCount(6)
 await page.locator('.sidebar nav a[href="#/"]').click()
 await page.evaluate(()=>document.documentElement.dataset.appearance='dark')
 await page.screenshot({path:'../docs/screenshots/home-window/default-dark.png',animations:'disabled'})
 await page.setViewportSize({width:config.minWidth,height:config.minHeight})
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
 await page.screenshot({path:'../docs/screenshots/home-window/minimum.png',animations:'disabled'})
})

for(const viewport of [{width:1348,height:1024},{width:1100,height:720},{width:620,height:640}]){
 test(`navigation keeps shell geometry and resets content scroll at ${viewport.width}px`,async({page})=>{
  await page.setViewportSize(viewport)
  await page.goto('http://127.0.0.1:1420/#/')
  await expect(page.locator('.usage-card')).toHaveCount(2)
  await page.evaluate(()=>document.fonts.ready)
  const initial=await geometry(page)
  for(const route of ['/settings','/','/tasks','/sessions','/usage','/settings','/']){
   await page.locator(`.sidebar nav a[href="#${route}"]`).click()
   await expect(page.locator('.desktop-shell')).toHaveAttribute('data-page',route)
   await expect.poll(()=>geometry(page)).toEqual(initial)
   const workspace=page.locator('.workspace')
   await workspace.evaluate(el=>{el.scrollTop=el.scrollHeight})
   if(route==='/settings'){
    if(viewport.width>=1024)expect(await workspace.evaluate(el=>el.scrollTop)).toBe(0)
    else expect(await workspace.evaluate(el=>el.scrollTop)).toBeGreaterThan(0)
    await page.screenshot({path:`../docs/screenshots/navigation/settings-scrolled-${viewport.width}.png`,animations:'disabled'})
   }
  }
  // An overflowing page must not narrow the content or move the persistent shell.
  await page.locator('.workspace').evaluate(el=>{el.scrollTop=0})
  await page.locator('.workspace main').evaluate(el=>{const filler=document.createElement('div');filler.style.height='2500px';el.append(filler)})
  expect(await geometry(page)).toEqual(initial)
  await page.locator('.workspace').evaluate(el=>{el.scrollTop=1200})
  await page.locator('.sidebar nav a[href="#/tasks"]').click()
  await expect.poll(()=>geometry(page)).toEqual(initial)
  await page.locator('.sidebar nav a[href="#/"]').click()
  await expect.poll(()=>geometry(page)).toEqual(initial)
  await page.screenshot({path:`../docs/screenshots/navigation/home-${viewport.width}.png`,animations:'disabled'})
 })
}
