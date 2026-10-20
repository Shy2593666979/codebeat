import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'

test.use({viewport:{width:1200,height:820},locale:'zh-CN',timezoneId:'Asia/Shanghai'})
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const n=new URL(route.request().url()).pathname,file=path.resolve('dist',n==='/'?'index.html':n.slice(1));if(!file.startsWith(path.resolve('dist')+path.sep)){await route.abort();return}await route.fulfill({path:file})})
 await installDesktopFixture(page)
})
async function pagedSessions(page:import('@playwright/test').Page){
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__sessionReadPages=[];g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{
  const result=await invoke(command,args);if(command!=='list_conversations')return result
  g.__sessionReadPages.push({provider:args.provider,cursor:args.cursor,search:args.search})
  const base=result.data[0]||{provider:args.provider,cwd:'/web/app',resumable:true,source:'fixture'},start=args.cursor?Number(args.cursor):0
  let data=Array.from({length:23},(_,i)=>({...base,session_id:args.provider+'-'+i,title:'会话 '+String(i).padStart(2,'0'),last_updated_utc:new Date(Date.UTC(2026,9,16,12,0)-i*60000-(args.provider==='codex'?0:30000)).toISOString()}))
  if(args.search)data=data.filter(c=>c.title.includes(args.search))
  const next=start+8<data.length?String(start+8):null
  return {data:data.slice(start,start+8),next_cursor:next,warnings:[]}
 }})
}
test('all backend pages feed real totals, numbered pages, page size and cross-page selection',async({page})=>{
 await pagedSessions(page);await page.goto('http://127.0.0.1:1420/#/sessions')
 const rows=page.locator('.sessions-table tbody tr'),footer=page.locator('.session-pagination')
 await expect(footer).toContainText('共 46 个会话，已选择 0 个');await expect(rows).toHaveCount(10)
 await page.evaluate(()=>document.fonts.ready);await page.screenshot({path:'../docs/screenshots/session-pagination/default.png',animations:'disabled'})
 await expect(page.getByRole('button',{name:'加载更多会话',exact:true})).toHaveCount(0)
 expect(await page.evaluate(()=>(window as any).__sessionReadPages.length)).toBe(6)
 await page.getByRole('checkbox',{name:'选择全部会话',exact:true}).check();await expect(footer).toContainText('已选择 10 个')
 await page.getByRole('button',{name:'下一页',exact:true}).click();await expect(rows.first()).toContainText('会话 05')
 await rows.first().getByRole('checkbox').check();await expect(footer).toContainText('已选择 11 个')
 await page.getByRole('button',{name:'第 5 页',exact:true}).click();await expect(rows).toHaveCount(6);await expect(page.getByRole('button',{name:'下一页',exact:true})).toBeDisabled()
 await page.getByRole('combobox',{name:'每页会话数'}).click();await page.getByRole('option',{name:'每页 20 条'}).click();await expect(rows).toHaveCount(20)
 const geometry=await page.locator('.session-table-panel>.table-scroll').evaluate(el=>({height:el.clientHeight,content:el.scrollHeight,overflow:getComputedStyle(el).overflowY}))
 expect(geometry.content).toBeLessThanOrEqual(geometry.height+1);expect(geometry.overflow).toBe('visible')
 await page.getByRole('button',{name:'第 1 页',exact:true}).click();await expect(rows.first()).toContainText('会话 00')
 await page.getByRole('button',{name:'清空选择',exact:true}).click();await expect(footer).toContainText('已选择 0 个')
 await page.screenshot({path:'../docs/screenshots/session-pagination/page-size-20.png',animations:'disabled'})
})
test('filters reset pagination and later-page actions use the actual selected conversation',async({page})=>{
 await pagedSessions(page);await page.goto('http://127.0.0.1:1420/#/sessions')
 await expect(page.locator('.session-pagination')).toContainText('共 46 个会话')
 await page.getByRole('button',{name:'第 3 页',exact:true}).click()
 const row=page.locator('.sessions-table tbody tr').first();await expect(row).toContainText('会话 10')
 await row.click({button:'right'});await page.getByRole('menuitem',{name:'会话详情',exact:true}).click()
 const detail=page.getByRole('dialog',{name:'会话详情',exact:true});await expect(detail.locator('.detail-heading h3')).toHaveText('会话 10')
 await page.keyboard.press('Escape')
 await page.getByRole('combobox',{name:'会话提供商',exact:true}).click();await page.getByRole('option',{name:'Claude Code',exact:true}).click();await page.clock.runFor(400)
 await expect(page.locator('.session-pagination')).toContainText('共 23 个会话');await expect(page.getByRole('button',{name:'上一页',exact:true})).toBeDisabled()
 await page.getByRole('button',{name:'第 2 页',exact:true}).click();await page.getByLabel('搜索会话',{exact:true}).fill('会话 22');await page.clock.runFor(400)
 await expect(page.locator('.sessions-table tbody tr')).toHaveCount(1);await expect(page.locator('.session-pagination')).toContainText('共 1 个会话')
})
test('failed pagination keeps available sessions and identifies the count as partial',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{
  if(command==='list_conversations'&&args.cursor)throw Error('page failed fixture')
  const result=await invoke(command,args);return command==='list_conversations'?{...result,next_cursor:'more'}:result
 }})
 await page.goto('http://127.0.0.1:1420/#/sessions')
 await expect(page.locator('.session-pagination')).toContainText('已加载 8 个会话')
 await expect(page.locator('.field-message').first()).toContainText('page failed fixture')
 await expect(page.locator('.sessions-table tbody tr')).toHaveCount(8)
})
test('pagination fits small and dark layouts without moving the application shell',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/sessions');await expect(page.locator('.session-pagination')).toContainText('共 8 个会话')
 for(const width of [1348,1200,1024,620]){
  await page.setViewportSize({width,height:820});expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
  if(width===1200){const footer=(await page.locator('.session-pagination').boundingBox())!;expect(footer.y+footer.height).toBeLessThanOrEqual(820)}
 }
 await page.evaluate(()=>document.documentElement.dataset.appearance='dark');await page.screenshot({path:'../docs/screenshots/session-pagination/small-dark.png',animations:'disabled'})
})

test('search invalidates an older in-flight backend page before the debounce completes',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{
  const result=await invoke(command,args)
  if(command!=='list_conversations'||args.search||args.provider==='claude_code')return result
  if(!args.cursor)return {...result,next_cursor:'old-page'}
  return new Promise(resolve=>{g.__resolveStalePage=()=>resolve({data:[{...result.data[0],session_id:'stale',title:'过期会话'}],next_cursor:null,warnings:[]})})
 }})
 await page.goto('http://127.0.0.1:1420/#/sessions');await expect.poll(()=>page.evaluate(()=>typeof(window as any).__resolveStalePage)).toBe('function')
 await page.getByLabel('搜索会话',{exact:true}).fill('Landing');await page.evaluate(()=>(window as any).__resolveStalePage());await page.clock.runFor(400)
 await expect(page.locator('.session-pagination')).toContainText('共 1 个会话');await expect(page.locator('.sessions-table tbody tr')).toHaveCount(1);await expect(page.locator('.sessions-table')).not.toContainText('过期会话')
})
test('provider response timing never flashes a partial provider list',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{
  const result=await invoke(command,args);if(command!=='list_conversations')return result
  if(args.provider==='claude_code'){g.__claudeReturned=true;return result}
  return new Promise(resolve=>{g.__releaseCodex=()=>resolve(result)})
 }})
 await page.goto('http://127.0.0.1:1420/#/sessions')
 await expect.poll(()=>page.evaluate(()=>!!(window as any).__claudeReturned&&typeof(window as any).__releaseCodex==='function')).toBe(true)
 await expect(page.locator('.sessions-table tbody tr')).toHaveCount(0)
 await expect(page.getByRole('heading',{name:'正在读取已有会话…',exact:true})).toBeVisible()
 await page.evaluate(()=>(window as any).__releaseCodex())
 await expect(page.locator('.session-pagination')).toContainText('共 8 个会话')
 await expect(page.locator('.sessions-table tbody tr')).toHaveCount(8)
 await expect(page.locator('.sessions-table tbody tr').first()).toContainText('实现用户认证系统')
 await expect(page.locator('.session-bottom')).toHaveCount(0)
 await expect(page.getByRole('heading',{name:'最近使用',exact:true})).toHaveCount(0)
 await expect(page.getByRole('heading',{name:'推荐会话',exact:true})).toHaveCount(0)
})

test('session footer matches task bottom spacing with full, short and empty pages',async({page})=>{
 await pagedSessions(page)
 await page.goto('http://127.0.0.1:1420/#/tasks')
 await expect(page.locator('.task-pagination')).toContainText('共 6 个任务')
 const taskFooter=(await page.locator('.task-pagination').boundingBox())!
 await page.getByRole('link',{name:'会话',exact:true}).click()
 const footer=page.locator('.session-pagination')
 await expect(footer).toContainText('共 46 个会话')
 async function aligned(){const box=(await footer.boundingBox())!;expect(Math.abs(box.y-taskFooter.y)).toBeLessThan(2);expect(Math.abs(box.y+box.height-taskFooter.y-taskFooter.height)).toBeLessThan(2)}
 await aligned()
 await page.getByRole('button',{name:'第 5 页',exact:true}).click();await expect(page.locator('.sessions-table tbody tr')).toHaveCount(6);await aligned()
 await page.getByLabel('搜索会话',{exact:true}).fill('不存在的会话');await page.clock.runFor(400)
 await expect(footer).toContainText('共 0 个会话');await aligned()
 await page.screenshot({path:'../docs/screenshots/session-pagination/empty-footer.png',animations:'disabled'})
 await page.getByLabel('搜索会话',{exact:true}).fill('');await page.clock.runFor(400);await expect(footer).toContainText('共 46 个会话');await aligned()
 await page.screenshot({path:'../docs/screenshots/session-pagination/default.png',animations:'disabled'})
})

test('one-minute cache restores page and selection instantly, then refreshes expired data in background',async({page})=>{
 await pagedSessions(page)
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__pendingSessionReads=[];g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{const result=await invoke(command,args);if(command==='list_conversations'&&g.__holdSessionReads)return new Promise(resolve=>g.__pendingSessionReads.push(()=>resolve(result)));return result}})
 await page.clock.pauseAt(new Date('2026-10-16T06:40:10Z'))
 await page.goto('http://127.0.0.1:1420/#/sessions');const footer=page.locator('.session-pagination')
 await expect(footer).toContainText('共 46 个会话');await page.getByRole('button',{name:'第 3 页',exact:true}).click();await page.locator('.sessions-table tbody tr').first().getByRole('checkbox').check()
 async function revisit(){await page.getByRole('link',{name:'首页',exact:true}).click();await page.getByRole('link',{name:'会话',exact:true}).click()}
 await revisit();await expect(footer).toContainText('已选择 1 个');await expect(page.getByRole('button',{name:'第 3 页',exact:true})).toHaveAttribute('aria-current','page')
 expect(await page.evaluate(()=>(window as any).__sessionReadPages.length)).toBe(6)
 await page.clock.fastForward(59_999);await revisit();expect(await page.evaluate(()=>(window as any).__sessionReadPages.length)).toBe(6)
 await page.clock.fastForward(1);await page.evaluate(()=>(window as any).__holdSessionReads=true);await revisit()
 await expect.poll(()=>page.evaluate(()=>(window as any).__pendingSessionReads.length)).toBe(2)
 await expect(page.locator('.sessions-table tbody tr')).toHaveCount(10);await expect(page.locator('.sessions-table tbody tr').first()).toContainText('会话 10');await expect(page.getByRole('button',{name:'下一页',exact:true})).toBeEnabled()
 await page.evaluate(()=>{const g=window as any;g.__holdSessionReads=false;g.__pendingSessionReads.forEach((resolve:any)=>resolve())})
 await expect.poll(()=>page.evaluate(()=>(window as any).__sessionReadPages.length)).toBe(12);await expect(footer).toContainText('共 46 个会话，已选择 1 个')
})

test('manual refresh bypasses fresh cache and failed refresh preserves the current list',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{if(command==='list_conversations'&&g.__failSessions)throw Error('refresh failed fixture');return invoke(command,args)}})
 await page.goto('http://127.0.0.1:1420/#/sessions');await expect(page.locator('.session-pagination')).toContainText('共 8 个会话')
 await page.getByRole('button',{name:'刷新会话',exact:true}).click();await expect(page.getByRole('button',{name:'刷新会话',exact:true})).toBeEnabled()
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>c.command==='list_conversations').length)).toBe(4)
 await page.evaluate(()=>(window as any).__failSessions=true);await page.getByRole('button',{name:'刷新会话',exact:true}).click()
 await expect(page.locator('.field-message').first()).toContainText('refresh failed fixture');await expect(page.locator('.sessions-table tbody tr')).toHaveCount(8)
})

test('empty results are cached and provider filters survive navigation without fetching again',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/sessions');await expect(page.locator('.session-pagination')).toContainText('共 8 个会话')
 await page.getByRole('combobox',{name:'会话提供商',exact:true}).click();await page.getByRole('option',{name:'Claude Code',exact:true}).click();await page.clock.runFor(400);await expect(page.locator('.sessions-table tbody tr')).toHaveCount(4)
 await page.getByLabel('搜索会话',{exact:true}).fill('no matching session');await page.clock.runFor(400);await expect(page.locator('.session-pagination')).toContainText('共 0 个会话')
 const count=await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>c.command==='list_conversations').length)
 await page.getByRole('link',{name:'首页',exact:true}).click();await page.getByRole('link',{name:'会话',exact:true}).click()
 await expect(page.getByLabel('搜索会话',{exact:true})).toHaveValue('no matching session');await expect(page.getByRole('combobox',{name:'会话提供商',exact:true})).toContainText('Claude Code');await expect(page.locator('.session-pagination')).toContainText('共 0 个会话')
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>c.command==='list_conversations').length)).toBe(count)
})

test('leaving during initial loading reuses the same request on return',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__pendingSessionReads=[];g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{const result=await invoke(command,args);if(command==='list_conversations')return new Promise(resolve=>g.__pendingSessionReads.push(()=>resolve(result)));return result}})
 await page.goto('http://127.0.0.1:1420/#/sessions');await expect.poll(()=>page.evaluate(()=>(window as any).__pendingSessionReads.length)).toBe(2)
 await page.getByRole('link',{name:'首页',exact:true}).click();await page.getByRole('link',{name:'会话',exact:true}).click()
 await page.evaluate(()=>(window as any).__pendingSessionReads.forEach((resolve:any)=>resolve()))
 await expect(page.locator('.session-pagination')).toContainText('共 8 个会话');expect(await page.evaluate(()=>(window as any).__pendingSessionReads.length)).toBe(2)
})
