import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'

test.use({viewport:{width:1200,height:820},locale:'zh-CN',timezoneId:'Asia/Shanghai'})
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const n=new URL(route.request().url()).pathname,file=path.resolve('dist',n==='/'?'index.html':n.slice(1));if(!file.startsWith(path.resolve('dist')+path.sep)){await route.abort();return}await route.fulfill({path:file})})
 await installDesktopFixture(page)
})
test('task reference table shows real counts and a five-action menu with details in a dialog',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/tasks')
 await expect(page.locator('.task-row')).toHaveCount(6)
 await expect(page.locator('.task-table th')).toHaveText(['','任务名称','提供商','执行计划','下次运行时间','状态','启用','操作'])
 await expect(page.locator('.task-page>aside')).toHaveCount(0);await expect(page.getByRole('dialog')).toHaveCount(0)
 await page.locator('.task-row').first().getByRole('checkbox').check();await expect(page.locator('.task-pagination')).toContainText('共 6 个任务，已选择 1 个')
 await page.locator('.task-menu-trigger').first().click()
 await expect(page.getByRole('menu').getByRole('menuitem')).toHaveText(['查看详情','编辑任务','立即运行','暂停任务','删除任务'])
 await page.evaluate(()=>document.fonts.ready);await page.screenshot({path:'../docs/screenshots/task-table/menu-default.png',animations:'disabled'})
 await page.getByRole('menuitem',{name:'查看详情'}).click()
 const detail=page.getByRole('dialog',{name:'任务详情',exact:true});await expect(detail.locator('.detail-heading h3')).toHaveText('Codex 自动继续任务')
 await page.screenshot({path:'../docs/screenshots/task-table/details.png',animations:'disabled'})
 await page.keyboard.press('Escape');await expect(detail).toHaveCount(0)
 await page.getByRole('combobox',{name:'任务提供商',exact:true}).click();await page.getByRole('option',{name:'Codex',exact:true}).click();await expect(page.locator('.task-row')).toHaveCount(3)
 await page.getByRole('button',{name:'已启用 2',exact:true}).click();await expect(page.locator('.task-row')).toHaveCount(2)
 await page.getByLabel('搜索任务名称或描述',{exact:true}).fill('接口');await expect(page.locator('.task-row')).toHaveCount(1)
})
test('pagination and page selection work across 23 actual records and clamp after filtering',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{const result=await invoke(command,args);if(command!=='list_tasks')return result;return Array.from({length:23},(_,i)=>({...result[0],id:'paged-'+i,request:{...result[0].request,title:'分页任务 '+i}}))}})
 await page.goto('http://127.0.0.1:1420/#/tasks');await expect(page.locator('.task-row')).toHaveCount(10)
 await page.getByRole('checkbox',{name:'选择本页全部任务'}).check();await expect(page.locator('.task-pagination')).toContainText('已选择 10 个')
 await page.getByRole('button',{name:'下一页',exact:true}).click();await expect(page.locator('.task-row h3').first()).toHaveText('分页任务 10')
 await page.locator('.task-row').first().getByRole('checkbox').check();await expect(page.locator('.task-pagination')).toContainText('已选择 11 个')
 await page.getByRole('button',{name:'第 3 页',exact:true}).click();await expect(page.locator('.task-row')).toHaveCount(3);await expect(page.getByRole('button',{name:'下一页',exact:true})).toBeDisabled()
 await page.getByRole('combobox',{name:'每页任务数'}).click();await page.getByRole('option',{name:'每页 20 条'}).click();await expect(page.locator('.task-row')).toHaveCount(20)
 await page.getByLabel('搜索任务名称或描述',{exact:true}).fill('分页任务 22');await expect(page.locator('.task-row')).toHaveCount(1);await expect(page.getByRole('button',{name:'上一页',exact:true})).toBeDisabled()
})
test('pause, resume, run, edit and delete target the operated row through production IPC',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/tasks')
 const trigger=page.locator('.task-menu-trigger').first()
 await trigger.click();await page.getByRole('menuitem',{name:'暂停任务'}).click();await expect(page.locator('.task-row').first().getByRole('switch')).toHaveAttribute('aria-checked','false')
 await page.getByRole('button',{name:'知道了',exact:true}).click();await page.clock.runFor(300)
 await trigger.click();await page.getByRole('menuitem',{name:'恢复任务'}).click();await expect(page.locator('.task-row').first().getByRole('switch')).toHaveAttribute('aria-checked','true')
 await page.getByRole('button',{name:'知道了',exact:true}).click();await page.clock.runFor(300)
 await trigger.click();await page.getByRole('menuitem',{name:'立即运行'}).click()
 await page.getByRole('button',{name:'知道了',exact:true}).click();await page.clock.runFor(300)
 await trigger.click();await page.getByRole('menuitem',{name:'编辑任务'}).click();await expect(page.getByRole('dialog',{name:'编辑任务',exact:true}).getByLabel('任务名称')).toHaveValue('Codex 自动继续任务');await page.keyboard.press('Escape')
 await trigger.click();await page.getByRole('menuitem',{name:'删除任务',exact:true}).click();await page.getByRole('button',{name:'确认删除',exact:true}).click();await expect(page.locator('.task-row')).toHaveCount(5)
 const calls=await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>['pause_task','run_task_now','cancel_task'].includes(c.command)))
 expect(calls.map((c:any)=>c.command)).toEqual(['pause_task','pause_task','run_task_now','cancel_task'])
 for(const call of calls)expect(call.args.id).toBe('b27dfaa2-8399-401d-9428-000000000000')
})
test('table stays within the window in dark and narrow layouts and menus support keyboard',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/tasks')
 for(const width of [1373,1200,1024,620]){
  await page.setViewportSize({width,height:820});await page.evaluate(()=>document.fonts.ready)
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
  const footer=(await page.locator('.task-pagination').boundingBox())!;expect(footer.y+footer.height).toBeLessThanOrEqual(820)
  if(width===1373||width===1200)await page.screenshot({path:`../docs/screenshots/task-table/table-${width}.png`,animations:'disabled'})
 }
 await page.evaluate(()=>document.documentElement.dataset.appearance='dark')
 await page.screenshot({path:'../docs/screenshots/task-table/small-dark.png',animations:'disabled'})
 const row=page.locator('.task-row').first();await row.focus();await page.keyboard.press('Shift+F10');await expect(page.getByRole('menuitem',{name:'查看详情'})).toBeFocused()
 await page.keyboard.press('End');await expect(page.getByRole('menuitem',{name:'删除任务',exact:true})).toBeFocused();await page.keyboard.press('Escape');await expect(row).toBeFocused()
})

test('the same task notification reopens details after closing or editing',async({page})=>{
 const target='/tasks?task=b27dfaa2-8399-401d-9428-000000000001'
 await page.goto('http://127.0.0.1:1420/#'+target)
 const detail=page.getByRole('dialog',{name:'任务详情',exact:true})
 await expect(detail.locator('.detail-heading h3')).toHaveText('Claude Code 自动继续任务')
 await page.keyboard.press('Escape');await expect(detail).toHaveCount(0);await expect(page).toHaveURL(/#\/tasks$/)
 await page.evaluate(target=>(window as any).__codebeatEmit('notification-open',target),target)
 await expect(detail).toBeVisible()
 await detail.getByRole('button',{name:'编辑任务',exact:true}).click();await expect(page.getByRole('dialog',{name:'编辑任务',exact:true})).toBeVisible();await expect(page).toHaveURL(/#\/tasks$/)
 await page.keyboard.press('Escape')
 await page.evaluate(target=>(window as any).__codebeatEmit('notification-open',target),target)
 await expect(detail.locator('.detail-heading h3')).toHaveText('Claude Code 自动继续任务')
})

test('task provider dropdown supports both tools and resets pagination',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/tasks')
 const filter=page.getByRole('combobox',{name:'任务提供商',exact:true})
 await expect(filter).toContainText('全部提供商');await expect(page.locator('.task-provider-tabs')).toHaveCount(0)
 await filter.click();await page.getByRole('option',{name:'Claude Code',exact:true}).click()
 await expect(page.locator('.task-row')).toHaveCount(3);await expect(page.locator('.task-provider-pill')).toHaveText(['Claude Code','Claude Code','Claude Code'])
 await filter.click();await page.getByRole('option',{name:'全部提供商',exact:true}).click();await expect(page.locator('.task-row')).toHaveCount(6)
 await page.screenshot({path:'../docs/screenshots/task-table/provider-filter.png',animations:'disabled'})
})

test('task search aligns with the title and table left edge at desktop widths',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/tasks')
 for(const width of [1200,1348,1024]){
  await page.setViewportSize({width,height:820})
  const positions=await page.evaluate(()=>['.task-page .page-heading h1','.task-search','.task-table-panel'].map(selector=>document.querySelector(selector)!.getBoundingClientRect().left))
  expect(Math.max(...positions)-Math.min(...positions)).toBeLessThan(1)
 }
 await page.setViewportSize({width:1200,height:820});await page.screenshot({path:'../docs/screenshots/task-table/search-aligned.png',animations:'disabled'})
})
