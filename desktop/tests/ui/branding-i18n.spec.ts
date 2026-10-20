import {test,expect,type Page} from '@playwright/test'
import path from 'node:path'
import {readFileSync,readdirSync} from 'node:fs'
import ts from 'typescript'
const messages:Record<string,string>=JSON.parse(readFileSync('src/locales/ui-en.json','utf8'))
import {installDesktopFixture} from './fixtures'

test.use({viewport:{width:1200,height:820},locale:'zh-CN',timezoneId:'Asia/Shanghai'})
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const n=new URL(route.request().url()).pathname,file=path.resolve('dist',n==='/'?'index.html':n.slice(1));if(!file.startsWith(path.resolve('dist')+path.sep)){await route.abort();return}await route.fulfill({path:file})})
 await installDesktopFixture(page)
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{
  const result=await invoke(command,args)
  if(command==='list_tasks')return result.map((task:any,index:number)=>({...task,request:{...task.request,title:'Task '+index,prompt:'Continue the project',conversation:{...task.request.conversation,title:'Session '+index}}}))
  if(command==='list_conversations')return {...result,data:result.data.map((c:any)=>({...c,title:'Session '+c.session_id.slice(-2)}))}
  if(command==='get_conversation_detail')return {...result,messages:[{role:'user',text:'Please continue this project.'},{role:'assistant',text:'The next step is ready.'}]}
  return result
 }})
})
async function noChinese(page:Page){
 const leftovers=await page.evaluate(()=>document.body.innerText.split('\n').map(s=>s.trim()).filter(s=>/[\u3400-\u9fff]/u.test(s)))
 expect(leftovers).toEqual([])
}
async function english(page:Page){
 await page.goto('http://127.0.0.1:1420/#/settings')
 await page.getByRole('combobox',{name:'界面语言',exact:true}).click();await page.getByRole('option',{name:'English',exact:true}).click();await page.clock.runFor(300)
 await expect(page.locator('html')).toHaveAttribute('lang','en-US')
}
test('brand colors match the footer and the tagline uses exactly two unclipped lines',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/')
 for(const width of [1348,1200,1024,980,800]){
  await page.setViewportSize({width,height:820})
  const colors=await page.locator('.brand-accent,.sidebar-bottom strong>span').evaluateAll(elements=>elements.map(el=>({color:getComputedStyle(el).color,background:getComputedStyle(el).backgroundImage})))
  expect(colors).toHaveLength(3);expect(new Set(colors.map(c=>c.color)).size).toBe(1)
  for(const c of colors)expect(c.background).toBe('none')
  const lines=page.locator('.sidebar-tagline-line');await expect(lines).toHaveText(['让 AI 编程的时间，','更有节奏'])
  for(const line of await lines.all())expect(await line.evaluate(el=>el.scrollWidth<=el.clientWidth)).toBe(true)
  const bounds=await lines.evaluateAll(elements=>elements.map(el=>el.getBoundingClientRect().height));expect(bounds[0]).toBeCloseTo(bounds[1],0)
  await expect(page.locator('.usage-link')).toHaveCount(0)
  if(width===1200)await page.screenshot({path:'../docs/screenshots/branding-i18n/home-zh.png',animations:'disabled'})
 }
})
test('English applies live to every page, dropdown and scheduling dialog',async({page})=>{
 await english(page)
 for(const [route,title] of [['/settings','Settings'],['/','Welcome to CodeBeat'],['/tasks','Task management'],['/sessions','Sessions'],['/usage','Usage monitoring'],['/history','History.'],['/providers','Providers.']]){
  await page.goto('http://127.0.0.1:1420/#'+route)
  await expect(page.getByRole('heading',{name:title,exact:true})).toBeVisible();await noChinese(page)
  if(route==='/'){
   await page.screenshot({path:'../docs/screenshots/branding-i18n/home-en.png',animations:'disabled'})
   await page.locator('.token-chart-title').click();await noChinese(page);await page.keyboard.press('Escape')
  }
  if(route==='/settings'){
   expect(await page.locator('.workspace').evaluate(el=>el.scrollHeight<=el.clientHeight)).toBe(true)
   await page.screenshot({path:'../docs/screenshots/branding-i18n/settings-en.png',animations:'disabled'})
   await page.getByRole('combobox',{name:'Appearance',exact:true}).click();await noChinese(page);await page.keyboard.press('Escape')
  }
 }
 await page.goto('http://127.0.0.1:1420/#/tasks')
 await page.evaluate(()=>(window as any).__sessionLockStates={'codex:a27dfaa2-8399-401d-9428-000000000000':'in_use'})
 await page.getByRole('button',{name:'New task',exact:true}).click()
 const editor=page.getByRole('dialog',{name:'New task',exact:true});await noChinese(page)
 await editor.getByRole('option').first().click()
 await expect(editor.getByRole('group',{name:'Session lock policy',exact:true})).toBeVisible()
 await expect(editor.getByRole('radio',{name:'Create a conversation fork',exact:true})).toBeChecked();await noChinese(page)
 await page.screenshot({path:'../docs/screenshots/branding-i18n/task-en.png',animations:'disabled'})
 await editor.getByRole('option').first().click();await editor.getByLabel('Task name',{exact:false}).fill('English task');await editor.getByLabel('Prompt',{exact:false}).fill('Continue existing work.')
 await editor.getByRole('button',{name:'Create task',exact:true}).click();await expect(editor).toHaveCount(0)
 await page.getByRole('button',{name:'Got it',exact:true}).click();await page.clock.runFor(300)
 const request=await page.evaluate(()=>(window as any).__codebeatCalls.find((c:any)=>c.command==='create_task').args.request)
 expect(request).toMatchObject({title:'English task',prompt:'Continue existing work.',consent:true,permission_profile:'full_access',locked_session_behavior:'fork'})
 await page.goto('http://127.0.0.1:1420/#/sessions');await page.locator('.sessions-table tbody tr').first().click({button:'right'});await noChinese(page)
 await page.getByRole('menuitem',{name:'Session details'}).click();await noChinese(page)
 await page.getByRole('button',{name:'Conversation',exact:true}).click();await noChinese(page)
 await page.keyboard.press('Escape');await page.getByRole('button',{name:'New session',exact:true}).click();await noChinese(page)
 await page.keyboard.press('Escape');await page.goto('http://127.0.0.1:1420/#/settings')
 await page.getByRole('combobox',{name:'Language',exact:true}).click();await page.getByRole('option',{name:'Chinese (Simplified)',exact:true}).click();await page.clock.runFor(300)
 await expect(page.getByRole('heading',{name:'设置',exact:true})).toBeVisible()
})

test('every explicit system message has an English translation',()=>{
 const files=['src/App.vue',...readdirSync('src/pages').filter(n=>n.endsWith('.vue')).map(n=>'src/pages/'+n),...readdirSync('src/components').filter(n=>n.endsWith('.vue')).map(n=>'src/components/'+n)]
 const missing=new Set<string>()
 for(const file of files){for(const match of readFileSync(file,'utf8').matchAll(/tr\((['"])(.*?)\1/g)){
  const ast=ts.createSourceFile('key.ts',match[0]+')',ts.ScriptTarget.Latest,true),node=ast.statements[0] as ts.ExpressionStatement,call=node.expression as ts.CallExpression,key=(call.arguments[0] as ts.StringLiteral).text.trim()
  if(/[\u3400-\u9fff]/u.test(key)&&!Object.hasOwn(messages,key))missing.add(key)
 }}
 expect([...missing]).toEqual([])
})

test('English keeps user-authored titles, conversation text and project paths unchanged',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{
  const result=await invoke(command,args)
  if(command==='list_tasks')result[0].request.title='用户自己的任务名称'
  if(command==='get_conversation_detail')result.messages=[{role:'user',text:'用户原始中文对话，不应翻译'}]
  return result
 }})
 await english(page);await page.goto('http://127.0.0.1:1420/#/tasks')
 await expect(page.locator('.task-row h3').first()).toHaveText('用户自己的任务名称')
 await page.goto('http://127.0.0.1:1420/#/sessions');await page.locator('.sessions-table tbody tr').first().click({button:'right'})
 await page.getByRole('menuitem',{name:'Session details'}).click();await page.getByRole('button',{name:'Conversation',exact:true}).click()
 await expect(page.locator('.conversation-content')).toContainText('用户原始中文对话，不应翻译')
})

test('tray follows the saved language instead of the browser locale',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{const result=await invoke(command,args);if(command==='get_tray_snapshot')result.settings.language='en-US';return result}})
 await page.goto('http://127.0.0.1:1420/#/tray')
 await expect(page.getByRole('menuitem',{name:'Open main window',exact:true})).toBeVisible();await noChinese(page)
 await page.screenshot({path:'../docs/screenshots/branding-i18n/tray-en.png',animations:'disabled'})
})

test('page titles share typography and distance below the titlebar',async({page})=>{
 for(const width of [1200,1348,1024,620]){
  await page.setViewportSize({width,height:820})
  const positions:number[]=[]
  for(const route of ['/','/tasks','/sessions','/usage','/settings']){
   await page.goto('http://127.0.0.1:1420/#'+route);await expect(page.locator('.page-heading h1')).toBeVisible()
   const metrics=await page.evaluate(()=>{
    const title=document.querySelector('.page-heading h1')!,description=document.querySelector('.page-heading p')!,bar=document.querySelector('.titlebar')!
    return {title:getComputedStyle(title).fontSize,description:getComputedStyle(description).fontSize,distance:title.getBoundingClientRect().top-bar.getBoundingClientRect().bottom}
   })
   expect(metrics.title).toBe('30px');expect(metrics.description).toBe('18px');positions.push(metrics.distance)
  }
  expect(Math.max(...positions)-Math.min(...positions)).toBeLessThan(1)
 }
})

test('home cards move up six pixels and fit without a vertical scrollbar at default size',async({page})=>{
 await page.setViewportSize({width:1200,height:820});await page.goto('http://127.0.0.1:1420/#/');await expect(page.locator('.home-bottom')).toBeVisible()
 const layout=await page.evaluate(()=>{const workspace=document.querySelector('.workspace')!,header=document.querySelector('.home-heading')!,cards=document.querySelector('.usage-grid')!;return {scrollHeight:workspace.scrollHeight,height:workspace.clientHeight,gap:cards.getBoundingClientRect().top-header.getBoundingClientRect().bottom,titleSize:getComputedStyle(header.querySelector('h1')!).fontSize}})
 expect(layout.gap).toBe(18);expect(layout.scrollHeight).toBeLessThanOrEqual(layout.height+1);expect(layout.titleSize).toBe('30px')
 await page.screenshot({path:'../docs/screenshots/header-30-18/home.png',animations:'disabled'})
})
