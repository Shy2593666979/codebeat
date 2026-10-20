import {test,expect} from '@playwright/test'
import path from 'node:path'
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{
  const name=new URL(route.request().url()).pathname,file=path.resolve('dist',name==='/'?'index.html':name.slice(1))
  if(!file.startsWith(path.resolve('dist')+path.sep)){await route.abort();return}
  await route.fulfill({path:file})
 })
})
test('five pages navigate with honest disconnected states',async({page})=>{
 const errors:string[]=[];page.on('pageerror',error=>errors.push(error.message))
 await page.goto('http://127.0.0.1:1420')
 await expect(page.getByRole('heading',{name:'欢迎使用 CodeBeat'})).toBeVisible()
 await expect(page.getByText('浏览器预览 · 未连接桌面服务')).toBeVisible()
 await expect(page.locator('.usage-card')).toHaveCount(2)
 await expect(page.locator('.usage-card').first()).toContainText('未知')
 await page.screenshot({path:'../docs/screenshots/overview.png',fullPage:true})
 for(const [route,title] of [['tasks','任务管理'],['sessions','会话记录'],['usage','额度监控'],['settings','设置']]){
  await page.locator('nav a[href="#/'+route+'"]').click();await expect(page.getByRole('heading',{name:title,exact:true})).toBeVisible()
 }
 expect(errors).toEqual([])
})
test('scheduling requires a resumable session without an authorization checkbox',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/tasks')
 await page.getByRole('button',{name:'新建任务'}).first().click()
 await expect(page.getByRole('button',{name:'确认创建'})).toBeDisabled()
 await page.locator('#task-title').fill('测试排程')
 await page.getByLabel('提示词').fill('检查测试')
 await expect(page.getByRole('dialog').getByRole('checkbox')).toHaveCount(0)
 await expect(page.getByRole('button',{name:'确认创建'})).toBeDisabled()
 await page.getByRole('button',{name:'关闭',exact:true}).click()
 await expect(page.locator('.editor-modal')).toHaveCount(0)
})
