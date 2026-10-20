import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'
test.use({locale:'zh-CN',timezoneId:'Asia/Shanghai',viewport:{width:1200,height:820}})
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const n=new URL(route.request().url()).pathname,file=path.resolve('dist',n==='/'?'index.html':n.slice(1));if(!file.startsWith(path.resolve('dist')+path.sep)){await route.abort();return}await route.fulfill({path:file})})
 await installDesktopFixture(page)
})
test('edit uses reference layout and confirms once without preview',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/tasks')
 await page.locator('.task-menu-trigger').first().click();await page.getByRole('menuitem',{name:'编辑任务',exact:true}).click()
 const editor=page.getByRole('dialog',{name:'编辑任务',exact:true})
 await expect(editor.getByLabel('任务名称')).toHaveValue('Codex 自动继续任务')
 await expect(editor.getByRole('option',{selected:true})).toHaveCount(1)
 await editor.getByLabel('任务名称').fill('更新后的任务')
 await editor.getByLabel('提示词',{exact:false}).fill('按照既有会话继续检查。')
 await page.evaluate(()=>document.fonts.ready)
 await page.screenshot({path:'../docs/screenshots/task-editor-reference/edit-default.png',animations:'disabled'})
 const left=(await editor.locator('.editor-session-column').boundingBox())!,right=(await editor.locator('.editor-fields-column').boundingBox())!,button=(await editor.getByRole('button',{name:'确认修改'}).boundingBox())!
 expect(left.x+left.width).toBeCloseTo(right.x+1,0);expect(button.y+button.height).toBeLessThan(820)
 await expect(editor.getByText('高级设置',{exact:true})).toHaveCount(0)
 await editor.getByRole('button',{name:'确认修改'}).click()
 await expect(page.getByRole('dialog')).toHaveCount(0)
 const calls=await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>c.command==='update_task'))
 expect(calls).toHaveLength(1);expect(calls[0].args.request.title).toBe('更新后的任务')
 await expect(page.getByRole('dialog',{name:'确认排程'})).toHaveCount(0)
})
test('failed direct submission preserves input and retries in the same editor',async({page})=>{
 await page.addInitScript(()=>{const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke;let failed=false;g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{if(command==='create_task'&&!failed){failed=true;throw Error('fixture registration failed')}return invoke(command,args)}})
 await page.goto('http://127.0.0.1:1420/#/tasks');await page.getByRole('button',{name:'新建任务',exact:true}).click()
 const editor=page.getByRole('dialog',{name:'新建任务',exact:true})
 await editor.getByRole('option').first().click();await editor.getByLabel('任务名称').fill('失败后保留任务');await editor.getByLabel('提示词',{exact:false}).fill('继续原有会话')
 await editor.getByRole('button',{name:'确认创建'}).click();await expect(editor.getByRole('alert')).toContainText('fixture registration failed')
 await expect(editor.getByLabel('任务名称')).toHaveValue('失败后保留任务');await expect(page.getByRole('dialog')).toHaveCount(1)
 await editor.getByRole('button',{name:'确认创建'}).click();await expect(page.getByRole('dialog')).toHaveCount(0)
})
test('immediate continuation confirms once and invokes the registered task',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/sessions')
 await page.locator('.sessions-table tbody tr').first().click({button:'right'})
 await page.getByRole('menuitem',{name:'立即继续',exact:true}).click()
 const editor=page.getByRole('dialog',{name:'立即继续会话',exact:true})
 await editor.getByLabel('提示词',{exact:false}).fill('继续处理当前会话中未完成的工作。')
 await editor.getByRole('button',{name:'立即继续',exact:true}).click()
 await expect(page.getByRole('dialog')).toHaveCount(0)
 const calls=await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>['create_task','run_task_now'].includes(c.command)))
 expect(calls.map((c:any)=>c.command)).toEqual(['create_task','run_task_now'])
 expect(calls[1].args.id).toBe('created-task')
})
test('session cards search, dark layout and sidebar has three solid dots',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/tasks')
 const bubble=page.locator('.sidebar nav a[href="#/sessions"] .conversation-bubble')
 await expect(bubble.locator('circle')).toHaveCount(3)
 for(const dot of await bubble.locator('circle').all()){expect(await dot.getAttribute('fill')).toBe('currentColor');expect(Number(await dot.getAttribute('r'))).toBeGreaterThan(1)}
 await page.screenshot({path:'../docs/screenshots/task-editor-reference/sidebar-dots.png',animations:'disabled'})
 await page.getByRole('button',{name:'新建任务',exact:true}).click()
 const editor=page.getByRole('dialog',{name:'新建任务',exact:true})
 await editor.getByLabel('搜索已有对话').fill('Landing');await page.clock.runFor(300);await expect(editor.getByRole('option')).toHaveCount(1)
 await editor.getByRole('option').click();await page.evaluate(()=>document.documentElement.dataset.appearance='dark')
 await page.screenshot({path:'../docs/screenshots/task-editor-reference/create-dark.png',animations:'disabled'})
 await page.setViewportSize({width:620,height:640})
 expect(await editor.evaluate(el=>el.scrollWidth<=el.clientWidth)).toBe(true)
 const footer=(await editor.locator('.editor-actions').boundingBox())!;expect(footer.y+footer.height).toBeLessThanOrEqual(640)
 await page.screenshot({path:'../docs/screenshots/task-editor-reference/create-small.png',animations:'disabled'})
})

for(const viewport of [{width:1200,height:800},{width:1600,height:1000},{width:620,height:640}]){
 test(`new task follows four fifths of current window ${viewport.width}x${viewport.height}`,async({page})=>{
  await page.setViewportSize(viewport)
  await page.goto('http://127.0.0.1:1420/#/tasks')
  await page.getByRole('button',{name:'新建任务',exact:true}).click()
  const editor=page.getByRole('dialog',{name:'新建任务',exact:true})
  await expect(editor).toBeVisible()
  await expect.poll(async()=>Math.abs((await editor.boundingBox())!.width-viewport.width*4/5)).toBeLessThan(0.5)
  const box=(await editor.boundingBox())!
  expect(box.width).toBeCloseTo(viewport.width*4/5,0)
  expect(box.height).toBeCloseTo(viewport.height*4/5,0)
  expect(box.x).toBeCloseTo((viewport.width-box.width)/2,0)
  expect(box.y).toBeCloseTo((viewport.height-box.height)/2,0)
  expect(await editor.evaluate(el=>el.scrollWidth<=el.clientWidth)).toBe(true)
  const footerBefore=(await editor.locator('.editor-actions').boundingBox())!
  await editor.locator('.editor-fields-scroll').evaluate(el=>el.scrollTop=el.scrollHeight)
  const footerAfter=(await editor.locator('.editor-actions').boundingBox())!
  expect(footerAfter.y).toBeCloseTo(footerBefore.y,0)
  expect(footerAfter.y+footerAfter.height).toBeLessThanOrEqual(box.y+box.height)
  const fields=await editor.locator('.editor-fields-scroll').evaluate(el=>({top:el.scrollTop,height:el.clientHeight,total:el.scrollHeight}))
  if(fields.total>fields.height)expect(fields.top).toBeGreaterThan(0)
  await page.screenshot({path:`../docs/screenshots/task-editor-reference/create-four-fifths-${viewport.width}.png`,animations:'disabled'})
  // 保持弹窗打开并拉伸主窗口，验证尺寸实时变化，而非只在打开时计算。
  const resized={width:viewport.width+180,height:viewport.height+120}
  await page.setViewportSize(resized)
  await expect.poll(async()=>Math.abs((await editor.boundingBox())!.width-resized.width*4/5)).toBeLessThan(0.5)
  const resizedBox=(await editor.boundingBox())!
  expect(resizedBox.height).toBeCloseTo(resized.height*4/5,0)
  expect(resizedBox.x).toBeCloseTo(resized.width/10,0)
  expect(resizedBox.y).toBeCloseTo(resized.height/10,0)
 })
}

for(const viewport of [{width:1024,height:700},{width:1200,height:800},{width:1600,height:1000},{width:620,height:640}]){
 test(`task fields fit without scrolling ${viewport.width}x${viewport.height}`,async({page})=>{
  await page.setViewportSize(viewport)
  await page.goto('http://127.0.0.1:1420/#/tasks')
  await page.getByRole('button',{name:'新建任务',exact:true}).click()
  const editor=page.getByRole('dialog',{name:'新建任务',exact:true})
  await expect(editor.getByRole('radio',{name:'创建聊天分叉',exact:true})).toBeChecked()
  await expect(editor.getByRole('radio',{name:'暂停任务',exact:true})).toBeVisible()
  await expect(editor.getByText('按本地时间执行',{exact:false})).toHaveCount(0)
  await editor.getByLabel('重复执行',{exact:true}).click()
  await page.getByRole('option',{name:'按小时间隔',exact:true}).click()
  const fit=await editor.locator('.editor-fields-scroll').evaluate(el=>({height:el.clientHeight,total:el.scrollHeight}))
  expect(fit.total).toBeLessThanOrEqual(fit.height+1)
  const policy=(await editor.locator('.session-lock-policy').boundingBox())!,footer=(await editor.locator('.editor-actions').boundingBox())!
  expect(policy.y+policy.height).toBeLessThanOrEqual(footer.y)
  await page.screenshot({path:`../docs/screenshots/task-editor-reference/compact-${viewport.width}.png`,animations:'disabled'})
 })
}
