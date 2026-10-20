import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'

const firstId='codex:a27dfaa2-8399-401d-9428-000000000000'
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{
  const name=new URL(route.request().url()).pathname
  await route.fulfill({path:path.resolve('dist',name==='/'?'index.html':name.slice(1))})
 })
 await installDesktopFixture(page)
 await page.addInitScript(id=>{(window as any).__sessionLockStates={[id]:'in_use'}},firstId)
})

test('occupied conversations show usage and a lock, remain selectable, and default to fork',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/sessions')
 await expect(page.locator('.sessions-table tbody tr').filter({hasText:'实现用户认证系统'}).locator('.badge')).toHaveText('使用中')
 await page.goto('http://127.0.0.1:1420/#/tasks')
 await page.getByRole('button',{name:'新建任务',exact:true}).click()
 const dialog=page.getByRole('dialog',{name:'新建任务',exact:true})
 const occupied=dialog.getByRole('option').filter({hasText:'实现用户认证系统'})
 await expect(occupied.getByLabel('使用中')).toBeVisible()
 await expect(dialog.locator('.session-option-chevron')).toHaveCount(0)
 await occupied.click()
 await expect(dialog.getByRole('radio',{name:'创建聊天分叉',exact:true})).toBeChecked()
 await expect(dialog.getByText('高级设置',{exact:true})).toHaveCount(0)
 const borders=await occupied.evaluate(el=>{const s=getComputedStyle(el);return [s.borderLeftWidth,s.borderRightWidth]})
 expect(borders[0]).toBe(borders[1])
 const background=await occupied.evaluate(el=>getComputedStyle(el).backgroundColor)
 const other=await dialog.getByRole('option').filter({hasText:'Landing page refactor'}).evaluate(el=>getComputedStyle(el).backgroundColor)
 expect(background).not.toBe(other)
 await dialog.getByLabel('任务名称').fill('占用会话分叉执行')
 await dialog.getByLabel('提示词',{exact:false}).fill('在分叉中继续检查')
 await dialog.locator('.session-lock-policy').scrollIntoViewIfNeeded()
 await page.screenshot({path:'../docs/screenshots/task-editor-reference/session-locked.png',animations:'disabled'})
 await dialog.getByRole('button',{name:'确认创建'}).click()
 await expect(dialog).toHaveCount(0)
 const request=await page.evaluate(()=>(window as any).__codebeatCalls.find((c:any)=>c.command==='create_task').args.request)
 expect(request).toMatchObject({locked_session_behavior:'fork',permission_profile:'full_access',notify:true,wake_to_run:true,catch_up_policy:'ask'})
})

test('pause is saved and the lock policy tracks occupancy while the editor remains open',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/tasks')
 await page.getByRole('button',{name:'新建任务',exact:true}).click()
 const dialog=page.getByRole('dialog',{name:'新建任务',exact:true})
 await dialog.getByRole('option').filter({hasText:'实现用户认证系统'}).click()
 const policy=dialog.getByRole('radio',{name:'暂停任务',exact:true})
 await policy.check()
 await expect(policy).toBeChecked()
 await dialog.getByLabel('任务名称').fill('等待会话空闲')
 await dialog.getByLabel('提示词',{exact:false}).fill('继续执行')
 await page.evaluate(id=>(window as any).__sessionLockStates[id]='unknown',firstId)
 await page.clock.runFor(5100)
 await expect(dialog.getByText('无法确认会话占用状态，请刷新后重试',{exact:true})).toBeVisible()
 await expect(dialog.getByRole('button',{name:'确认创建'})).toBeDisabled()
 await page.evaluate(id=>(window as any).__sessionLockStates[id]='available',firstId)
 await page.clock.runFor(5100)
 await expect(policy).toBeVisible()
 await expect(policy).toBeChecked()
 await expect(dialog.locator('.session-option-lock')).toHaveCount(0)
 // 提交时再变为占用，仍须保存此前选定的暂停策略，不回退为分叉。
 await page.evaluate(id=>(window as any).__sessionLockStates[id]='in_use',firstId)
 await dialog.getByRole('button',{name:'确认创建'}).click()
 await expect(dialog).toHaveCount(0)
 const request=await page.evaluate(()=>(window as any).__codebeatCalls.find((c:any)=>c.command==='create_task').args.request)
 expect(request.locked_session_behavior).toBe('pause')
})
