import {createRequire} from 'node:module'
import {pathToFileURL} from 'node:url'
const desktopRequire=createRequire(new URL('../desktop/package.json',import.meta.url))
const {chromium}=desktopRequire('@playwright/test')
import path from 'node:path'
import fs from 'node:fs/promises'
// Load the TypeScript fixture through the project's Vite loader, not Node's
// native TS loader (which cannot resolve its extensionless application imports).
const {createServer}=await import(pathToFileURL(desktopRequire.resolve('vite')).href)
const loader=await createServer({root:path.resolve('desktop'),configFile:false,optimizeDeps:{noDiscovery:true,include:[]},server:{middlewareMode:true},appType:'custom'})
let installDesktopFixture
try{({installDesktopFixture}=await loader.ssrLoadModule('/tests/ui/fixtures.ts'))}finally{await loader.close()}

const out=path.resolve('docs/screenshots/fonts')
await fs.mkdir(out,{recursive:true})
await fs.mkdir('docs/logs',{recursive:true})
const browser=await chromium.launch({channel:'chromium',headless:true,args:['--no-proxy-server']})
try{
 const page=await browser.newPage({viewport:{width:1344,height:1046},locale:'zh-CN',timezoneId:'Asia/Shanghai'})
 await page.route('http://127.0.0.1:1420/**',async route=>{
  const pathname=new URL(route.request().url()).pathname,file=path.resolve('desktop/dist',pathname==='/'?'index.html':pathname.slice(1))
  if(!file.startsWith(path.resolve('desktop/dist')+path.sep))return route.abort()
  await route.fulfill({path:file})
 })
 await installDesktopFixture(page)
 await page.goto('http://127.0.0.1:1420/#/settings')
 await page.locator('.setting-row strong').first().waitFor()
 await page.evaluate(()=>document.fonts.ready)
 const cdp=await page.context().newCDPSession(page)
 await cdp.send('DOM.enable');await cdp.send('CSS.enable')
 const {root}=await cdp.send('DOM.getDocument')
 const records=[]
 for(const selector of ['h1','.page-heading p','.sidebar nav a.active>span','.setting-row strong','.setting-row small','.brand']){
  const {nodeId}=await cdp.send('DOM.querySelector',{nodeId:root.nodeId,selector})
  if(!nodeId){records.push({selector,available:false});continue}
  const fonts=await cdp.send('CSS.getPlatformFontsForNode',{nodeId})
  const style=await page.locator(selector).first().evaluate(el=>{const s=getComputedStyle(el);return {text:el.textContent,fontFamily:s.fontFamily,fontWeight:s.fontWeight,fontSize:s.fontSize,lineHeight:s.lineHeight}})
  records.push({selector,...style,...fonts})
 }
 await page.screenshot({path:path.join(out,'current.png'),animations:'disabled'})
 for(const [name,family] of [['yahei-ui','"Microsoft YaHei UI","Segoe UI",sans-serif'],['yahei','"Microsoft YaHei","Segoe UI",sans-serif'],['noto-sans','"Noto Sans SC","Segoe UI",sans-serif']]){
  const style=await page.addStyleTag({content:`:root{font-family:${family}}`})
  await page.evaluate(()=>document.fonts.ready)
  await page.screenshot({path:path.join(out,name+'.png'),animations:'disabled'})
  const {nodeId}=await cdp.send('DOM.querySelector',{nodeId:root.nodeId,selector:'.setting-row strong'})
  records.push({candidate:name,fontFamily:family,...await cdp.send('CSS.getPlatformFontsForNode',{nodeId})})
  await style.evaluate(el=>el.remove())
 }
 await fs.writeFile('docs/logs/font-audit.json',JSON.stringify(records,null,2))
 const options=[['before-fonts','Before: system Segoe UI + Microsoft YaHei UI'],['current','Current: bundled CodeBeat UI (Noto Sans SC derived)'],['yahei-ui','Microsoft YaHei UI'],['yahei','Microsoft YaHei'],['noto-sans','Installed Noto Sans SC']]
 await fs.writeFile(path.join(out,'index.html'),`<!doctype html><meta charset="utf-8"><title>CodeBeat typography comparison</title><style>body{font:16px "Segoe UI",sans-serif;margin:24px;background:#eaf3fc;color:#142344}h1{font-size:24px}.grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:24px}figure{margin:0}img{width:100%;border:1px solid #a8c3df}figcaption{padding:10px 0}p{line-height:1.6}</style><h1>CodeBeat typography comparison</h1><p>Reference first, followed by the current implementation and local-font candidates. All captures use a 1344×1046 viewport. Current and candidate captures share the calibrated build; candidates change only font-family. Before shows the previous production typography. Font identity in the reference has not been confirmed.</p><figure><figcaption>Reference window client area</figcaption><img src="../comparison/settings-reference.png"></figure><div class="grid">${options.map(([name,label])=>`<figure><figcaption>${label}</figcaption><img src="${name}.png"></figure>`).join('')}</div>`)
 console.log(JSON.stringify(records,null,2))
}finally{await browser.close()}
