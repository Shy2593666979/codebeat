import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'

// 宣传截图仅使用浏览器夹具，不连接真实桌面服务、会话或模型。
test.use({viewport:{width:1440,height:940},locale:'zh-CN',timezoneId:'Asia/Shanghai',deviceScaleFactor:1})

test('capture the README product gallery with demonstration data',async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{
  const name=new URL(route.request().url()).pathname,file=path.resolve('dist',name==='/'?'index.html':name.slice(1))
  if(!file.startsWith(path.resolve('dist')+path.sep)){await route.abort();return}
  await route.fulfill({path:file})
 })
 await installDesktopFixture(page)
 await page.addInitScript(()=>{
  const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke
  const titles=['继续完成登录与注册','生成晚间回归报告','整理项目开发文档','重构支付接口校验','巡检 API 与依赖','补全设置页交互细节']
  const prompts=['沿用已有方案补全表单校验，运行测试并整理变更说明。','运行项目回归测试，汇总失败用例与建议修复步骤。','梳理现有接口与启动流程，更新 README 和开发说明。','检查支付参数与异常分支，补充相关测试。','检查关键接口与依赖状态，整理巡检摘要。','完成设置项交互，检查浅色与深色模式。']
  g.__TAURI_INTERNALS__.invoke=async(command:string,args:any={})=>{
   if(command==='get_optimizer_status')return {plans:[{provider:'codex',date:'2026-10-16',status:'active',day_start:'2026-10-15T23:30:00Z',display_times:['2026-10-15T23:30:00Z','2026-10-16T04:30:00Z','2026-10-16T09:30:00Z'],slots:[{display_at:'2026-10-16T09:30:00Z',execute_at:'2026-10-16T09:32:00Z'}]}],windows:[{provider:'codex',source:'app',start:'2026-10-15T23:30:00Z',end:'2026-10-16T04:30:00Z',display_start:'2026-10-15T23:30:00Z'},{provider:'codex',source:'observed',start:'2026-10-16T04:31:00Z',end:'2026-10-16T09:31:00Z'}]}
   const result=await invoke(command,args)
   if(command==='list_tasks')return result.map((task:any,i:number)=>({...task,request:{...task.request,title:titles[i],prompt:prompts[i],permission_profile:'full_access',catch_up_policy:'ask',recurrence:i===0?null:i===2?{kind:'weekly'}:task.request.recurrence,conversation:{...task.request.conversation,cwd:'C:\\Projects\\demo-app'}}}))
   if(command==='list_conversations')return {...result,data:result.data.map((c:any)=>({...c,cwd:'C:\\Projects\\demo-app'}))}
   if(command==='get_settings')return {...result,authorized_roots:['C:\\Projects'],connection_interval_minutes:10}
   if(command==='get_usage_refresh')return result.map((r:any)=>r.provider==='claude_code'?{...r,report:{provider:'claude_code',status:'unavailable',message:'暂无稳定的官方额度读取接口',windows:[]}}:{...r,report:{...r.report,reset_credits:{available_count:2,account_key:null},windows:[{label:'五小时窗口',duration_minutes:300,used_percent:28,resets_at_utc:'2026-10-16T09:31:00Z',observed_at_utc:'2026-10-16T06:40:00Z',source:'demo',reliability:'official'},{label:'每周额度',duration_minutes:10080,used_percent:36,resets_at_utc:'2026-10-20T02:00:00Z',observed_at_utc:'2026-10-16T06:40:00Z',source:'demo',reliability:'official'}]}})
   return result
  }
 })
 const shot=async(name:string)=>{await page.evaluate(()=>document.fonts.ready);await page.mouse.move(0,0);await page.screenshot({path:`../docs/images/readme/${name}.png`,animations:'disabled'})}
 await page.setViewportSize({width:1440,height:800})
 await page.goto('http://127.0.0.1:1420/#/')
 await expect(page.locator('.usage-card')).toHaveCount(2)
 await expect(page.locator('.today-panel')).toBeVisible()
 await shot('home')
 await page.setViewportSize({width:1440,height:940})
 await page.goto('http://127.0.0.1:1420/#/tasks')
 await expect(page.locator('.task-row')).toHaveCount(6)
 await shot('tasks')
 await page.getByRole('button',{name:'新建任务',exact:true}).click()
 const editor=page.getByRole('dialog',{name:'新建任务',exact:true})
 await editor.getByRole('option').first().click()
 await editor.getByLabel('任务名称').fill('今晚继续，完成登录流程')
 await editor.getByLabel('提示词',{exact:false}).fill('继续当前会话中的登录与注册功能。\n补全表单校验和错误提示，运行相关测试。\n完成后整理变更与需要我确认的事项。')
 await editor.getByLabel('执行时间').fill('2026-10-16T19:30')
 await editor.getByLabel('执行时间').blur()
 await shot('new-task')
 await editor.getByRole('button',{name:'关闭',exact:true}).click()
 await page.setViewportSize({width:1440,height:900})
 await page.goto('http://127.0.0.1:1420/#/usage')
 await expect(page.locator('.usage-card')).toHaveCount(2)
 await shot('usage')
 await page.setViewportSize({width:1440,height:940})
 await page.goto('http://127.0.0.1:1420/#/settings')
 const allocation=page.locator('.pretrigger-settings')
 await expect(allocation.locator('.probe-times b')).toHaveText(['07:30','12:30','17:30'])
 await page.evaluate(()=>document.fonts.ready)
 await allocation.screenshot({path:'../docs/images/readme/coding-allocation.png',animations:'disabled'})
 await page.goto('http://127.0.0.1:1420/#/tasks')
 await page.evaluate(()=>document.documentElement.dataset.appearance='dark')
 await shot('tasks-dark')
})
