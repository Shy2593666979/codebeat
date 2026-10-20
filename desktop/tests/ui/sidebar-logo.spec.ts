import {test,expect} from '@playwright/test'
import path from 'node:path'
import {readFileSync} from 'node:fs'
import {installDesktopFixture} from './fixtures'

test.use({viewport:{width:1200,height:820}})
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const n=new URL(route.request().url()).pathname,file=path.resolve('dist',n==='/'?'index.html':n.slice(1));if(!file.startsWith(path.resolve('dist')+path.sep)){await route.abort();return}await route.fulfill({path:file})})
 await installDesktopFixture(page)
})

test('wave decoration stays clipped and the user SVG preserves its navy color and transparency',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/')
 for(const width of [1348,1200,1024,980,620]){
  await page.setViewportSize({width,height:820})
  const decoration=page.locator('.desktop-shell>.sidebar-decoration'),sidebar=(await page.locator('.sidebar').boundingBox())!,clip=(await decoration.boundingBox())!
  await expect(decoration.locator('.backdrop-art')).toHaveCount(1)
  await expect(page.locator('.desktop-shell>.backdrop-art')).toHaveCount(0)
  expect(clip.x).toBeGreaterThanOrEqual(sidebar.x);expect(clip.x+clip.width).toBeLessThanOrEqual(sidebar.x+sidebar.width)
  expect(await decoration.evaluate(el=>getComputedStyle(el).overflow)).toBe('hidden')
  expect(await decoration.evaluate(el=>getComputedStyle(el).pointerEvents)).toBe('none')
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
  if(width===1200){await page.evaluate(()=>document.fonts.ready);await page.screenshot({path:'../docs/screenshots/sidebar-logo/home.png',animations:'disabled'})}
 }
 await page.setViewportSize({width:1200,height:820})
 const palette=await page.locator('.brand .brand-mark').evaluate(async img=>{
  const image=img as HTMLImageElement;await image.decode();const canvas=document.createElement('canvas');canvas.width=image.naturalWidth;canvas.height=image.naturalHeight
  const context=canvas.getContext('2d')!;context.drawImage(image,0,0);const data=context.getImageData(0,0,canvas.width,canvas.height).data
  let solid=0,navy=0;for(let i=0;i<data.length;i+=4)if(data[i+3]>240){solid++;if(data[i]===17&&data[i+1]===16&&data[i+2]===50)navy++}
  return {solid,navy,cornerAlpha:data[3],textColor:getComputedStyle(document.querySelector('.brand-accent')!).color}
 })
 expect(palette.solid).toBeGreaterThan(500);expect(palette.navy/palette.solid).toBeGreaterThan(.99);expect(palette.cornerAlpha).toBe(0);expect(palette.textColor).toBe('rgb(0, 120, 255)')
 await page.locator('.sidebar nav a[href="#/settings"]').click();await page.screenshot({path:'../docs/screenshots/sidebar-logo/settings.png',animations:'disabled'})
 await page.evaluate(()=>document.documentElement.dataset.appearance='dark');await page.screenshot({path:'../docs/screenshots/sidebar-logo/dark.png',animations:'disabled'})
})

test('logo comparison shows prior concept and original user SVG without modifying the source',async({page})=>{
 expect(readFileSync('src/assets/app-logo.svg').equals(readFileSync('../docs/images/logo.svg'))).toBe(true)
 const old='data:image/png;base64,'+readFileSync('assets/brand/concepts/zzz-sleep-v3.png').toString('base64'),current='data:image/png;base64,'+readFileSync('assets/brand/logo.png').toString('base64')
 await page.setViewportSize({width:1040,height:620})
 await page.setContent(`<style>*{box-sizing:border-box}body{margin:0;background:#edf6ff;color:#102448;font:16px "Segoe UI",sans-serif}main{padding:28px;display:grid;grid-template-columns:1fr 1fr;gap:20px}section{border:1px solid #dcecff;border-radius:18px;background:white;padding:24px}h2{font-size:19px;margin:0 0 16px}img{object-fit:contain}.brand{display:flex;align-items:center;gap:12px;font-size:26px;font-weight:700}.brand img{width:50px;height:50px}.blue{color:#0078ff}.large{display:block;width:170px;height:170px;margin:12px auto}.sizes{display:flex;align-items:center;justify-content:center;gap:24px;margin-top:16px}.dark{display:flex;align-items:center;justify-content:center;gap:24px;background:#172b43;border-radius:12px;margin-top:20px;padding:20px;color:#dbeaff}.dark img{width:44px;height:44px}p{color:#7189ac;font-size:13px;text-align:center;margin:18px 0 0}</style><main>${[[old,'旧版 · 睡眠标志'],[current,'用户 SVG · 原色 #111032']].map(([src,title])=>`<section><h2>${title}</h2><div class="brand"><img src="${src}"><span>Code<span class="blue">Beat</span></span></div><img class="large" src="${src}"><div class="sizes">${[16,24,32,48].map(size=>`<img src="${src}" width="${size}" height="${size}">`).join('')}</div><div class="dark"><img src="${src}"><strong>Code<span class="blue">Beat</span></strong></div><p>透明背景 · 原始文件轮廓与颜色</p></section>`).join('')}</main>`)
 await page.locator('img').evaluateAll(images=>Promise.all(images.map(img=>(img as HTMLImageElement).decode())))
 await page.screenshot({path:'../docs/screenshots/sidebar-logo/logo-comparison.png',animations:'disabled'})
})
