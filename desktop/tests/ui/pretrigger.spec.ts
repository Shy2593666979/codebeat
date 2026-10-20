import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const name=new URL(route.request().url()).pathname;await route.fulfill({path:path.resolve('dist',name==='/'?'index.html':name.slice(1))})})
 await installDesktopFixture(page)
})
test('coding quota is enabled by default, has fixed policy, and saves schedule changes',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/settings')
 const card=page.locator('.pretrigger-settings')
 await expect(card.getByRole('heading',{name:'每日Coding 额度调配'})).toBeVisible()
 await expect(card.getByRole('switch',{name:'每日Coding 额度调配'})).toHaveAttribute('aria-checked','true')
 await expect(card.locator('.coding-allocation-icon')).toBeVisible()
 await expect(card.locator('details')).toHaveCount(0)
 await expect(card.getByText('午休不暂停窗口计时；实际恢复以官方数据为准。')).toHaveCount(0)
 await expect(card.getByRole('combobox',{name:'优化模式',exact:true})).toHaveCount(0)
 await expect(card.getByRole('switch',{name:'自动轻量触发',exact:true})).toHaveCount(0)
 await card.getByRole('combobox',{name:'预启动生效日期'}).click();await page.getByRole('option',{name:'每天（含周末）',exact:true}).click()
 await card.getByLabel('工作开始时间',{exact:true}).fill('09:00');await page.clock.runFor(300)
 const saved=await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>c.command==='update_settings').at(-1).args.settings)
 expect(saved.pretrigger_enabled).toBe(true);expect(saved.pretrigger).toMatchObject({mode:'balanced',auto_trigger:true,wake:true,work_start:'09:00',days:'daily'})
 await card.screenshot({path:'../docs/screenshots/pretrigger/coding-quota.png',animations:'disabled'})
 await card.getByRole('switch',{name:'每日Coding 额度调配'}).click();await page.clock.runFor(300)
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.filter((c:any)=>c.command==='update_settings').at(-1).args.settings.pretrigger_enabled)).toBe(false)
})
test('small dark card has no overflow and never marks predictions as triggered',async({page})=>{
 await page.setViewportSize({width:620,height:740});await page.goto('http://127.0.0.1:1420/#/settings')
 await page.evaluate(()=>document.documentElement.dataset.appearance='dark')
 const card=page.locator('.pretrigger-settings');await expect(card.locator('.probe-times>.confirmed')).toHaveCount(0)
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
 await card.screenshot({path:'../docs/screenshots/pretrigger/coding-quota-dark.png',animations:'disabled'})
})
