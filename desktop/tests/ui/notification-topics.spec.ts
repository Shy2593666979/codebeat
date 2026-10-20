import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const name=new URL(route.request().url()).pathname;await route.fulfill({path:path.resolve('dist',name==='/'?'index.html':name.slice(1))})})
 await installDesktopFixture(page)
})
test('provider toggles replace the same card while other providers and quota events remain separate',async({page})=>{
 await page.addInitScript(()=>Object.defineProperty(document,'hidden',{configurable:true,get:()=>true}))
 await page.goto('http://127.0.0.1:1420/#/')
 const cards=page.locator('.feedback-card:not(.feedback-leave-active)'),codex=page.getByRole('switch',{name:'Codex 自动继续',exact:true})
 await codex.click();await expect(cards).toHaveCount(1)
 const identity=await cards.first().getAttribute('data-feedback-id')
 for(let i=0;i<12;i++){
  await codex.click()
  await expect(cards).toHaveCount(1)
  await expect(cards.first()).toContainText(i%2===0?'已恢复该工具的后续调度':'已暂停该工具的后续调度')
  expect(await cards.first().getAttribute('data-feedback-id')).toBe(identity)
 }
 await page.getByRole('switch',{name:'Claude Code 自动继续',exact:true}).click();await expect(cards).toHaveCount(2)
 await page.evaluate(()=>{const g=window as any;g.__inboxNotices.push({id:'d27dfaa2-8399-401d-9428-000000000999',topic:'provider:codex:quota-exhausted',title:'Codex 额度提醒',message:'额度已耗尽',kind:'info',page:'usage',task_id:null,at:new Date().toISOString()});g.__codebeatEmit('notices-changed',structuredClone(g.__inboxNotices))})
 await expect(cards).toHaveCount(3);await expect(cards.last()).toContainText('额度已耗尽')
 expect(await page.evaluate(()=>(window as any).__inboxNotices.length)).toBe(3)
 await page.screenshot({path:'../docs/screenshots/notifications/topic-replacement.png',animations:'disabled'})
})
test('updating a seen card cancels the old countdown and preserves its position',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/');await page.evaluate(()=>document.fonts.ready)
 const toggle=page.getByRole('switch',{name:'Codex 自动继续',exact:true}),card=page.locator('.feedback-card:not(.feedback-leave-active)')
 await toggle.click();await expect(card).toBeVisible();await card.hover();await expect(card.locator('.feedback-countdown')).toBeVisible()
 await page.clock.runFor(1000)
 const id=await card.getAttribute('data-feedback-id')
 await toggle.click();await expect(card).toContainText('已恢复该工具的后续调度');expect(await card.getAttribute('data-feedback-id')).toBe(id)
 await expect(card.locator('.feedback-countdown')).toHaveCount(0)
 await page.clock.runFor(1100);await expect(card).toBeVisible()
 await card.hover();await expect(card.locator('.feedback-countdown')).toBeVisible();await page.clock.runFor(2100);await expect(card).toHaveCount(0)
})
test('token chart and task pages use provider terminology',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/');await expect(page.getByRole('combobox',{name:'趋势提供商',exact:true})).toContainText('全部提供商')
 await page.goto('http://127.0.0.1:1420/#/tasks');await expect(page.locator('.task-table thead')).toContainText('提供商');await expect(page.locator('main')).not.toContainText('提供方')
})
