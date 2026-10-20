import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'

test.use({viewport:{width:1200,height:820}})
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const n=new URL(route.request().url()).pathname,file=path.resolve('dist',n==='/'?'index.html':n.slice(1));if(!file.startsWith(path.resolve('dist')+path.sep)){await route.abort();return}await route.fulfill({path:file})})
 await installDesktopFixture(page)
})

test('task picker automatically exhausts pages and deduplicates sessions',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__sessionPages=[];g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{
  if(command!=='list_conversations')return invoke(command,args)
  g.__sessionPages.push(args.cursor)
  const result=await invoke(command,args),base=result.data[0]
  const start=args.cursor===null?0:args.cursor==='second'?30:60
  return {data:Array.from({length:31},(_,i)=>({...base,session_id:'session-'+(start+i),title:'会话 '+(start+i)})),next_cursor:start===0?'second':start===30?'third':null,warnings:[]}
 }})
 await page.goto('http://127.0.0.1:1420/#/tasks');await page.getByRole('button',{name:'新建任务',exact:true}).click()
 const editor=page.getByRole('dialog')
 await expect(editor.getByRole('option')).toHaveCount(91)
 await expect(editor.getByRole('button',{name:'加载更多',exact:true})).toHaveCount(0)
 expect(await page.evaluate(()=>(window as any).__sessionPages)).toEqual([null,'second','third'])
 await editor.getByRole('option').last().click();await expect(editor.getByRole('option').last()).toHaveAttribute('aria-selected','true')
})

test('task picker stops repeated cursors and retains available sessions on error',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{const result=await invoke(command,args);if(command==='list_conversations')return {...result,next_cursor:'repeat'};return result}})
 await page.goto('http://127.0.0.1:1420/#/tasks');await page.getByRole('button',{name:'新建任务',exact:true}).click()
 const editor=page.getByRole('dialog');await expect(editor.getByRole('alert')).toContainText('分页游标重复')
 await expect(editor.getByRole('option')).toHaveCount(4)
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>c.command==='list_conversations').length)).toBe(2)
})

test('usage rows share the same column edges at desktop and small widths',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/usage')
 for(const width of [1348,1200,1100,980,620]){
  await page.setViewportSize({width,height:820})
  const cards=page.locator('.usage-grid>.usage-card'),lower=page.locator('.usage-middle>section')
  await expect(cards).toHaveCount(2);await expect(lower).toHaveCount(2)
  for(let i=0;i<2;i++){const a=(await cards.nth(i).boundingBox())!,b=(await lower.nth(i).boundingBox())!;expect(b.x).toBeCloseTo(a.x,0);expect(b.x+b.width).toBeCloseTo(a.x+a.width,0)}
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
  const title=(await page.locator('.usage-middle .token-chart-title').boundingBox())!,controls=(await page.locator('.usage-middle .token-controls').boundingBox())!
  expect(controls.y>=title.y+title.height-1||controls.x>=title.x+title.width-1).toBe(true)
  if(width===1200)await page.screenshot({path:'../docs/screenshots/usage-aligned.png',animations:'disabled'})
 }
})

test('hovering settings switches keeps their appearance and state stable without saving',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/settings')
 const switches=page.getByRole('switch')
 for(const appearance of ['light','dark']){
  await page.evaluate(value=>document.documentElement.dataset.appearance=value,appearance)
  for(const toggle of await switches.all()){
   await page.mouse.move(0,0)
   const read=()=>toggle.evaluate(el=>{const s=getComputedStyle(el),t=getComputedStyle(el.firstElementChild!),r=el.getBoundingClientRect();return {background:s.background,transform:s.transform,thumb:t.transform,x:r.x,y:r.y,width:r.width,height:r.height,state:el.getAttribute('aria-checked')}})
   const before=await read();await toggle.hover()
   for(let i=0;i<3;i++){await page.clock.runFor(200);expect(await read()).toEqual(before)}
  }
 }
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>c.command==='update_settings').length)).toBe(0)
 const sound=page.getByRole('switch',{name:'播放提示音'});await sound.click();await page.clock.runFor(300)
 await expect(sound).toHaveAttribute('aria-checked','false')
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>c.command==='update_settings').length)).toBe(1)
 await sound.focus();await page.keyboard.press('Space');await page.clock.runFor(300);await expect(sound).toHaveAttribute('aria-checked','true')
})

test('search invalidates an in-flight older page before its debounce completes',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__oldPagePending=false;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{
  if(command!=='list_conversations')return invoke(command,args)
  const result=await invoke(command,args)
  if(args.search)return result
  if(!args.cursor)return {...result,next_cursor:'old-page'}
  g.__oldPagePending=true
  return new Promise(resolve=>{g.__resolveOldPage=()=>resolve({data:[{...result.data[0],session_id:'stale-session',title:'过期结果'}],next_cursor:null,warnings:[]})})
 }})
 await page.goto('http://127.0.0.1:1420/#/tasks');await page.getByRole('button',{name:'新建任务',exact:true}).click()
 await expect.poll(()=>page.evaluate(()=>(window as any).__oldPagePending)).toBe(true)
 const editor=page.getByRole('dialog');await editor.getByLabel('搜索已有对话').fill('Landing')
 await page.evaluate(()=>(window as any).__resolveOldPage())
 await expect(editor.getByRole('option',{name:'过期结果',exact:false})).toHaveCount(0)
 await page.clock.runFor(300);await expect(editor.getByRole('option')).toHaveCount(1)
 await expect(editor.getByRole('option')).toContainText('Landing page refactor')
})
