import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'

test.use({viewport:{width:1348,height:1024},locale:'zh-CN',timezoneId:'Asia/Shanghai'})
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const name=new URL(route.request().url()).pathname,file=path.resolve('dist',name==='/'?'index.html':name.slice(1));if(!file.startsWith(path.resolve('dist')+path.sep)){await route.abort();return}await route.fulfill({path:file})})
 await installDesktopFixture(page)
})

test('numeric preferences save numbers including zero and support keyboard search',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/settings')
 const reminder=page.getByRole('combobox',{name:'默认提醒提前',exact:true})
 await reminder.focus();await page.keyboard.press('Enter')
 const search=page.getByRole('combobox',{name:'搜索默认提醒提前',exact:true})
 await expect(search).toBeFocused();await search.fill('关闭');await page.keyboard.press('Enter')
 await expect(reminder).toHaveText('关闭');await expect(reminder).toBeFocused()
 await page.getByRole('combobox',{name:'自动检测连接状态',exact:true}).click()
 await page.getByRole('option',{name:'每 10 分钟',exact:true}).click()
 await page.clock.runFor(300)
 await expect(page.locator('.settings-auto-state')).toHaveCount(0)
 const saved=await page.evaluate(()=>(window as any).__codebeatCalls.findLast((c:any)=>c.command==='update_settings').args.settings)
 expect(saved.reminder_minutes).toBe(0);expect(saved.connection_interval_minutes).toBe(10)
})

test('search, no results, Tab and Escape work inside the dialog without closing it',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/tasks');await page.getByRole('button',{name:'新建任务',exact:true}).click()
 const dialog=page.getByRole('dialog',{name:'新建任务',exact:true}),repeat=dialog.getByRole('combobox',{name:'重复执行',exact:true})
 await repeat.click()
 const search=page.getByRole('combobox',{name:'搜索重复执行',exact:true})
 await search.fill('没有此选项');await expect(page.getByRole('status').filter({hasText:'没有匹配选项'})).toBeVisible()
 await page.keyboard.press('ArrowDown');await page.keyboard.press('Enter');await expect(repeat).toContainText('仅执行一次')
 await page.keyboard.press('Escape');await expect(dialog).toBeVisible();await expect(repeat).toBeFocused();await expect(page.getByRole('listbox',{name:'重复执行',exact:true})).toHaveCount(0)
 await repeat.click();await page.keyboard.press('Tab');await expect(dialog.getByRole('radio',{name:'创建聊天分叉',exact:true})).toBeFocused()
 await repeat.click();await page.evaluate(()=>document.fonts.ready)
 await page.screenshot({path:'../docs/screenshots/select/task-repeat.png',animations:'disabled'})
 const option=page.getByRole('option',{name:'每天',exact:true})
 const box=(await option.boundingBox())!
 expect(box.x).toBeGreaterThanOrEqual(0);expect(box.y).toBeGreaterThanOrEqual(0);expect(box.y+box.height).toBeLessThanOrEqual(1024)
 expect(await option.evaluate(el=>{const b=el.getBoundingClientRect();return el.contains(document.elementFromPoint(b.x+b.width/2,b.y+b.height/2))})).toBe(true)
 await page.keyboard.press('Escape');await expect(dialog).toBeVisible();await page.keyboard.press('Escape');await expect(dialog).toHaveCount(0)
})

test('all filters use the shared control; values and dark/small positioning remain usable',async({page})=>{
 const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message))
 await page.goto('http://127.0.0.1:1420/#/sessions')
 const provider=page.getByRole('combobox',{name:'会话提供商',exact:true})
 await provider.click();await page.getByRole('option',{name:'Claude Code',exact:true}).click();await page.clock.runFor(500)
 await expect(page.locator('.sessions-table tbody tr')).toHaveCount(4)
 await page.getByRole('combobox',{name:'会话项目',exact:true}).click();await page.getByRole('combobox',{name:'搜索会话项目',exact:true}).fill('/web/app')
 await page.getByRole('option',{name:'/web/app',exact:true}).click();await expect(page.locator('.sessions-table tbody tr')).toHaveCount(1)
 await page.goto('http://127.0.0.1:1420/#/usage');await page.getByRole('combobox',{name:'趋势时间范围',exact:true}).click();await page.getByRole('option',{name:'近 14 天',exact:true}).click()
 await expect(page.locator('.chart-day')).toHaveCount(14)
 await page.goto('http://127.0.0.1:1420/#/history');await page.getByRole('combobox',{name:'运行结果筛选',exact:true}).click();await page.getByRole('option',{name:'失败',exact:true}).click();await expect(page.getByText('还没有运行记录')).toBeVisible()
 await page.goto('http://127.0.0.1:1420/#/settings')
 await page.setViewportSize({width:620,height:640})
 await page.evaluate(()=>document.documentElement.dataset.appearance='dark')
 await page.getByRole('combobox',{name:'自动检测连接状态',exact:true}).click()
 await page.screenshot({path:'../docs/screenshots/select/settings-small-dark.png',animations:'disabled'})
 const list=page.getByRole('listbox',{name:'自动检测连接状态',exact:true}),box=(await list.boundingBox())!
 expect(box.x).toBeGreaterThanOrEqual(0);expect(box.y).toBeGreaterThanOrEqual(0);expect(box.x+box.width).toBeLessThanOrEqual(620);expect(box.y+box.height).toBeLessThanOrEqual(640)
 await page.keyboard.press('Escape');await expect(list).toHaveCount(0)
 await page.getByRole('combobox',{name:'系统主题',exact:true}).click()
 await page.locator('.workspace').evaluate(el=>{el.scrollTop=el.scrollHeight;el.dispatchEvent(new Event('scroll'))})
 await expect(page.locator('.select-panel')).toHaveCount(0)
 await page.getByRole('combobox',{name:'系统主题',exact:true}).click();await page.getByRole('heading',{name:'设置',exact:true}).click();await expect(page.locator('.select-panel')).toHaveCount(0)
 expect(await page.locator('select').count()).toBe(0);expect(errors).toEqual([])
})
