import {test,expect} from '@playwright/test'
import path from 'node:path'
import {readFileSync} from 'node:fs'
import {installDesktopFixture} from './fixtures'
test.use({viewport:{width:1200,height:820},locale:'zh-CN'})
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const n=new URL(route.request().url()).pathname;await route.fulfill({path:path.resolve('dist',n==='/'?'index.html':n.slice(1))})})
 await installDesktopFixture(page)
})
async function cornerAlpha(page:import('@playwright/test').Page){
 const data=(await page.screenshot({omitBackground:true,animations:'disabled'})).toString('base64')
 return page.evaluate(async data=>{const image=new Image();image.src='data:image/png;base64,'+data;await image.decode();const canvas=document.createElement('canvas');canvas.width=image.width;canvas.height=image.height;const ctx=canvas.getContext('2d')!;ctx.drawImage(image,0,0);return [[0,0],[image.width-1,0],[0,image.height-1],[image.width-1,image.height-1]].map(([x,y])=>ctx.getImageData(x,y,1,1).data[3])},data)
}
test('normal window corners and dialog overlays are truly transparent',async({page})=>{
 const config=JSON.parse(readFileSync('src-tauri/tauri.conf.json','utf8')),main=config.app.windows.find((w:any)=>w.label==='main')
 expect(main.transparent).toBe(true);expect(main.backgroundColor).toBe('#00000000')
 await page.goto('http://127.0.0.1:1420/#/');await expect(page.locator('html')).toHaveAttribute('data-window-state','normal');await page.evaluate(()=>document.fonts.ready)
 expect(await cornerAlpha(page)).toEqual([0,0,0,0])
 await page.screenshot({path:'../docs/screenshots/window-corners/home.png',omitBackground:true,animations:'disabled'})
 await page.evaluate(()=>(window as any).__codebeatEmit('exit-requested',null));await expect(page.getByRole('heading',{name:'退出 CodeBeat',exact:true})).toBeVisible()
 expect(await cornerAlpha(page)).toEqual([0,0,0,0])
})
test('maximize and fullscreen square the corners and restore returns rounded corners',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/');await expect(page.locator('html')).toHaveAttribute('data-window-state','normal')
 await page.evaluate(()=>{const g=window as any;g.__windowMaximized=true;g.__codebeatEmit('tauri://resize',{width:1200,height:820})})
 await expect(page.locator('html')).toHaveAttribute('data-window-state','maximized');expect(await cornerAlpha(page)).toEqual([255,255,255,255])
 await page.evaluate(()=>{const g=window as any;g.__windowMaximized=false;g.__codebeatEmit('tauri://resize',{width:1200,height:820})})
 await expect(page.locator('html')).toHaveAttribute('data-window-state','normal');expect(await cornerAlpha(page)).toEqual([0,0,0,0])
 await page.evaluate(()=>{const g=window as any;g.__windowFullscreen=true;g.__codebeatEmit('tauri://resize',{width:1200,height:820})})
 await expect(page.locator('html')).toHaveAttribute('data-window-state','maximized')
})

test('exit checkbox aligns with its label and returning never cancels tasks',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/')
 await page.evaluate(()=>document.fonts.ready)
 for(const width of [1200,620]){
  await page.setViewportSize({width,height:800})
  await page.evaluate(()=>(window as any).__codebeatEmit('exit-requested',null))
  const dialog=page.getByRole('dialog',{name:'退出 CodeBeat',exact:true}),checkbox=dialog.getByRole('checkbox',{name:'同时取消所有尚未执行的任务',exact:true})
  await expect(checkbox).not.toBeChecked()
  const input=(await checkbox.boundingBox())!,label=(await dialog.locator('.exit-option>span').boundingBox())!,heading=(await dialog.getByRole('heading').boundingBox())!
  expect(Math.abs(input.y+input.height/2-label.y-label.height/2)).toBeLessThan(1)
  expect(label.x-input.x-input.width).toBeCloseTo(9,1)
  expect(input.x).toBeCloseTo(heading.x,1)
  await dialog.locator('.exit-option>span').click();await expect(checkbox).toBeChecked()
  await page.screenshot({path:`../docs/screenshots/window-corners/exit-aligned-${width}.png`,animations:'disabled'})
  await dialog.getByRole('button',{name:'返回',exact:true}).click();await expect(dialog).toHaveCount(0)
  expect(await page.evaluate(()=>(window as any).__codebeatCalls.some((c:any)=>c.command==='exit_app'))).toBe(false)
  // Reset only the local checkbox state for the next viewport.
  await page.evaluate(()=>(window as any).__codebeatEmit('exit-requested',null));await checkbox.uncheck()
  await dialog.getByRole('button',{name:'返回',exact:true}).click()
 }
})
