import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'
test.use({viewport:{width:1348,height:1024},locale:'zh-CN',timezoneId:'Asia/Shanghai'})
test.beforeEach(async({page})=>{await page.route('http://127.0.0.1:1420/**',async route=>{const name=new URL(route.request().url()).pathname,file=path.resolve('dist',name==='/'?'index.html':name.slice(1));if(!file.startsWith(path.resolve('dist')+path.sep)){await route.abort();return}await route.fulfill({path:file})});await installDesktopFixture(page)})
test('five reference layouts render using test-only fixtures without console errors',async({page})=>{
 const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message))
 for(const [route,name,title] of [['/','home','欢迎使用 CodeBeat'],['/tasks','tasks','任务管理'],['/sessions','sessions','会话记录'],['/usage','usage','额度监控'],['/settings','settings','设置']]){
  const sizes:Record<string,{width:number;height:number}>={home:{width:1328,height:998},tasks:{width:1373,height:1019},sessions:{width:1350,height:1019},usage:{width:1349,height:1024},settings:{width:1344,height:1046}}
  await page.setViewportSize(sizes[name])
  await page.goto('http://127.0.0.1:1420/#'+route);await expect(page.getByRole('heading',{name:title,exact:true})).toBeVisible()
  await page.evaluate(()=>document.fonts.ready)
  if(route==='/sessions'){await expect(page.locator('.sessions-table tbody tr')).toHaveCount(8);await expect(page.locator('.sessions-layout>.session-detail')).toHaveCount(0)}
  if(route==='/')await expect(page.locator('.usage-card').first()).toContainText('剩余 80%')
  await page.screenshot({path:'../docs/screenshots/redesign/'+name+'.png',fullPage:true})
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true)
 }
 expect(errors).toEqual([])
})
test('task filters, pause control and details use the real IPC contract',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/tasks');await expect(page.locator('.task-row')).toHaveCount(6)
 await page.getByRole('combobox',{name:'任务提供商',exact:true}).click();await page.getByRole('option',{name:'Codex',exact:true}).click();await expect(page.locator('.task-row')).toHaveCount(3)
 await page.getByRole('switch',{name:'Codex 自动继续任务 启用'}).click();await expect(page.getByRole('switch',{name:'Codex 自动继续任务 启用'})).toHaveAttribute('aria-checked','false')
 expect(await page.evaluate(()=>(window as any).__codebeatCalls.some((c:any)=>c.command==='pause_task'&&c.args.paused))).toBe(true)
})

test('Chinese and Latin labels render with packaged fonts at their real weights',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/settings');await page.evaluate(()=>document.fonts.ready)
 const faces=await page.evaluate(async()=>{await Promise.all([400,500,600,700].map(weight=>document.fonts.load(`${weight} 16px "CodeBeat UI"`,'中文 CodeBeat')));return Array.from(document.fonts).filter(font=>font.family==='CodeBeat UI').map(font=>font.status)})
 expect(faces).toHaveLength(4);expect(faces.every(status=>status==='loaded')).toBe(true)
 const cdp=await page.context().newCDPSession(page);await cdp.send('DOM.enable');await cdp.send('CSS.enable')
 const {root}=await cdp.send('DOM.getDocument')
 for(const [selector,weight] of [['h1','700'],['.setting-row strong','500'],['.settings-page .page-heading p','400'],['.brand','700']]){
  const {nodeId}=await cdp.send('DOM.querySelector',{nodeId:root.nodeId,selector})
  const {fonts}=await cdp.send('CSS.getPlatformFontsForNode',{nodeId})
  expect(fonts.length).toBeGreaterThan(0)
  for(const font of fonts){expect(font.isCustomFont).toBe(true);expect(font.familyName).toBe('CodeBeat UI')}
  expect(await page.locator(selector).first().evaluate(el=>getComputedStyle(el).fontWeight)).toBe(weight)
 }
})

test('notification task link selects its task and clears previous filters',async({page})=>{
 await page.addInitScript(()=>{(window as any).__codebeatLaunchTarget='/tasks?task=b27dfaa2-8399-401d-9428-000000000003'})
 await page.goto('http://127.0.0.1:1420/#/')
 await expect(page.locator('.task-detail h3')).toHaveText('凌晨构建任务')
 await page.getByLabel('搜索任务名称或描述').fill('Codex')
 await page.evaluate(()=>{location.hash='/tasks?task=b27dfaa2-8399-401d-9428-000000000005'})
 await expect(page.locator('.task-detail h3')).toHaveText('文档整理任务')
 await expect(page.getByLabel('搜索任务名称或描述')).toHaveValue('')
})

test('notification navigation does not wait for a slow provider detection',async({page})=>{
 await page.addInitScript(()=>{
  const globals=window as any,invoke=globals.__TAURI_INTERNALS__.invoke
  globals.__codebeatLaunchTarget='/tasks?task=b27dfaa2-8399-401d-9428-000000000001'
  globals.__TAURI_INTERNALS__.invoke=(command:string,args:any)=>command==='list_providers'?new Promise(()=>{}):invoke(command,args)
 })
 await page.goto('http://127.0.0.1:1420/#/')
 await expect(page.locator('.task-detail h3')).toHaveText('Claude Code 自动继续任务')
})

test('provider scheduling feedback appears in the top-right card and can be repeated',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/')
 const toggle=page.getByRole('switch',{name:'Codex 自动继续',exact:true})
 await expect(toggle).toBeEnabled()
 await toggle.click()
 await expect(page.locator('.feedback-card:not(.feedback-leave-active)')).toContainText('已暂停该工具的后续调度')
 await page.locator('.feedback-card:not(.feedback-leave-active)').getByRole('button',{name:'知道了'}).click()
 await page.clock.runFor(300)
 await toggle.click()
 const notice=page.locator('.feedback-card:not(.feedback-leave-active)')
 await expect(notice).toContainText('已恢复该工具的后续调度')
 const box=(await notice.boundingBox())!
 expect(box.x).toBeGreaterThan(800);expect(box.y).toBeLessThan(80)
 await expect(page.locator('.banner')).toHaveCount(0)
 await page.screenshot({path:'../docs/screenshots/notifications/provider-feedback.png',animations:'disabled'})
 await page.mouse.move(20,500);await page.clock.fastForward(120000);await expect(notice).toBeVisible();await expect(notice).not.toContainText(/已读|未读/)
 await expect(notice.locator('.feedback-countdown')).toHaveCount(0)
 await page.clock.pauseAt(await page.evaluate(()=>Date.now()+100))
 await notice.hover();await expect(notice).not.toContainText(/已读|未读/)
 const countdown=notice.locator('.feedback-countdown span')
 await expect(countdown).toBeVisible()
 expect(await countdown.evaluate(el=>getComputedStyle(el).animationDuration)).toBe('2s')
 // Inspect a deterministic midpoint without advancing the dismissal clock.
 await countdown.evaluate(el=>{for(const animation of el.getAnimations()){animation.pause();animation.currentTime=1000}})
 const fill=await countdown.evaluate(el=>Number(new DOMMatrixReadOnly(getComputedStyle(el).transform).a))
 expect(fill).toBeCloseTo(.5,2)
 await page.screenshot({path:'../docs/screenshots/notifications/countdown.png'})
 await page.clock.fastForward(1900);await expect(notice).toBeVisible()
 await page.clock.fastForward(101);await expect(notice).toHaveCount(0)
 await toggle.click();await expect(page.locator('.feedback-card:not(.feedback-leave-active)')).toContainText('已暂停该工具的后续调度')
 await page.locator('.feedback-card:not(.feedback-leave-active)').getByRole('button',{name:'关闭通知',exact:true}).click();await page.clock.runFor(300);await expect(page.locator('.feedback-card:not(.feedback-leave-active)')).toHaveCount(0)
 await toggle.click();await expect(page.locator('.feedback-card:not(.feedback-leave-active)')).toContainText('已恢复该工具的后续调度')
})

test('background and clipped feedback stays unread until focused and fully visible',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/')
 await page.getByRole('switch',{name:'Codex 自动继续',exact:true}).click()
 const notice=page.locator('.feedback-card:not(.feedback-leave-active)')
 await page.evaluate(()=>Object.defineProperty(document,'hasFocus',{configurable:true,value:()=>false}))
 await notice.hover();await page.clock.fastForward(20000);await expect(notice).not.toContainText(/已读|未读/)
 // Apply clipping before restoring focus so an after-enter/font-ready callback cannot acknowledge the full card between these changes.
 await page.locator('.feedback-notices').evaluate(el=>{(el as HTMLElement).style.maxHeight='60px';Object.defineProperty(document,'hasFocus',{configurable:true,value:()=>true})})
 await notice.getByRole('heading').hover();await page.clock.fastForward(20000);await expect(notice).not.toContainText(/已读|未读/)
 await page.locator('.feedback-notices').evaluate(el=>{(el as HTMLElement).style.maxHeight=''})
 await notice.getByRole('button',{name:'知道了'}).focus();await expect(notice).not.toContainText(/已读|未读/)
 await page.clock.fastForward(1900);await expect(notice).toBeVisible();await page.clock.fastForward(101);await expect(notice).toHaveCount(0)
})

test('operation errors use a red card and leave room for retry',async({page})=>{
 await page.addInitScript(()=>{const globals=window as any,invoke=globals.__TAURI_INTERNALS__.invoke;globals.__TAURI_INTERNALS__.invoke=(command:string,args:any)=>command==='update_settings'?Promise.reject('无法保存设置：数据库暂时不可用 <script>literal</script>'):invoke(command,args)})
 await page.goto('http://127.0.0.1:1420/#/')
 await page.getByRole('switch',{name:'Codex 自动继续',exact:true}).click()
 const notice=page.locator('.feedback-card.error:not(.feedback-leave-active)')
 await expect(notice).toHaveAttribute('role','alert')
 await expect(notice).toContainText('无法保存设置：数据库暂时不可用 <script>literal</script>')
 await expect(notice.locator('script')).toHaveCount(0)
 await expect(page.getByRole('switch',{name:'Codex 自动继续',exact:true})).toHaveAttribute('aria-checked','true')
 await notice.getByRole('button',{name:'关闭',exact:true}).click();await page.clock.runFor(300);await expect(notice).toHaveCount(0)
})
test('session search, content tabs, selection and scheduling work',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/sessions');await expect(page.locator('.sessions-table tbody tr')).toHaveCount(8)
 await page.locator('.sessions-table tbody tr').first().click({button:'right'});await page.getByRole('menuitem',{name:'会话详情',exact:true}).click()
 await page.getByRole('button',{name:'对话内容',exact:true}).click();await expect(page.locator('.conversation-content')).toContainText('JWT')
 await page.getByRole('button',{name:'创建定时任务',exact:true}).click();await expect(page.getByLabel('任务名称')).toHaveValue('继续「实现用户认证系统」');await expect(page.getByRole('button',{name:'确认创建'})).toBeDisabled()
 await page.getByRole('button',{name:'关闭',exact:true}).click();await page.getByLabel('搜索会话',{exact:true}).fill('Landing');await page.clock.fastForward(500);await expect(page.locator('.sessions-table tbody tr')).toHaveCount(1)
})
test('settings apply silently across navigation without extra settings controls',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/settings')
 await page.getByRole('switch',{name:'播放提示音'}).click()
 await page.clock.runFor(300)
 await expect(page.locator('.settings-auto-state')).toHaveCount(0)
 await expect(page.locator('.feedback-card')).toHaveCount(0)
 await expect(page.locator('.settings-savebar')).toHaveCount(0)
 await expect(page.getByRole('button',{name:'保存设置',exact:true})).toHaveCount(0)
 await page.locator('.sidebar nav a[href="#/"]').click()
 await page.locator('.sidebar nav a[href="#/settings"]').click()
 await expect(page.getByRole('switch',{name:'播放提示音'})).toHaveAttribute('aria-checked','false')
 await expect(page.locator('.settings-more')).toHaveCount(0)
 await expect(page.getByText('更改自动生效',{exact:true})).toHaveCount(0)
})
