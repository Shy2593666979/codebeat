import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'

test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{
  const name=new URL(route.request().url()).pathname
  await route.fulfill({path:path.resolve('dist',name==='/'?'index.html':name.slice(1))})
 })
 await installDesktopFixture(page)
 // Mock IPC only: these clicks can NEVER reach an installed Codex engine.
 await page.addInitScript(()=>{
  const g=window as any,original=g.__TAURI_INTERNALS__.invoke
  g.__resetFixture={count:2,outcome:'reset',timeout:false,keys:[]}
  g.__TAURI_INTERNALS__.invoke=async(command:string,args:any={})=>{
   const f=g.__resetFixture
   if(command==='refresh_usage'){
    const r=await original(command,args)
    return {...r,reset_credits:args.id==='codex'?{available_count:f.count,account_key:'fixture-account'}:null}
   }
   if(command==='get_usage_refresh'){const states=await original(command,args);return states.map((s:any)=>({...s,report:s.report?{...s.report,reset_credits:s.provider==='codex'?{available_count:f.count,account_key:'fixture-account'}:null}:null}))}
   if(command==='consume_reset_credit'){
    f.keys.push(args.idempotencyKey)
    if(f.timeout)throw Error('mock timeout')
    if(f.outcome==='reset')f.count--
    return f.outcome
   }
   return original(command,args)
  }
 })
})

test('home places reset time and matching credit icon side by side below quota',async({page})=>{
 await page.setViewportSize({width:1200,height:800})
 await page.goto('http://127.0.0.1:1420/#/')
 const card=page.locator('.usage-card.codex'),control=card.getByTestId('reset-credits')
 await expect(control).toContainText('2 张')
 await expect(page.locator('.usage-card .source-note')).toHaveCount(0)
 const quota=await card.locator('.quota-windows').boundingBox(),next=await card.locator('.next-compact').boundingBox(),box=await control.boundingBox()
 expect(next!.y).toBeGreaterThan(quota!.y+quota!.height)
 expect(box!.x).toBeGreaterThanOrEqual(next!.x+next!.width)
 expect(Math.abs(next!.y-box!.y)).toBeLessThan(2)
 const icon=await control.locator('.reset-credit-icon').innerHTML()
 await expect(page.locator('.usage-card.claude_code').getByTestId('reset-credits')).toHaveCount(0)
 expect(await page.evaluate(()=>(window as any).__resetFixture.keys)).toEqual([])
 await page.screenshot({path:'../docs/screenshots/reset-card-layout/home.png',fullPage:true})
 await page.locator('.sidebar nav a[href="#/usage"]').click()
 const summary=page.locator('.usage-card.codex .quota-summary')
 await expect(summary.locator(':scope>div')).toHaveCount(3)
 await expect(summary.locator(':scope>div').nth(0)).toContainText('下次重置')
 await expect(summary.locator(':scope>div').nth(1)).toContainText('重置卡')
 await expect(summary.locator(':scope>div').nth(2)).toContainText('数据状态')
 await expect(summary).not.toContainText('剩余额度')
 await expect(page.locator('.usage-card .source-note')).toHaveCount(0)
 expect(await summary.locator('.reset-credit-icon').innerHTML()).toBe(icon)
 await page.screenshot({path:'../docs/screenshots/reset-card-layout/usage.png',fullPage:true})
})
test('mock redemption refreshes count and survives navigation',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/usage')
 const control=page.getByTestId('reset-credits')
 await expect(control).toContainText('2 张')
 await control.getByRole('button',{name:'使用重置卡',exact:true}).click()
 await expect(control).toContainText('1 张')
 await expect(control).toContainText('重置卡已使用')
 await page.screenshot({path:'../docs/screenshots/reset-card-layout/usage-mock-redeemed.png',fullPage:true})
 expect(await page.evaluate(()=>(window as any).__resetFixture.keys.length)).toBe(1)
})
test('mock timeout retries same operation and zero disables use',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/')
 const control=page.getByTestId('reset-credits'),button=control.getByRole('button',{name:'使用重置卡',exact:true})
 await expect(button).toBeEnabled()
 await page.evaluate(()=>{(window as any).__resetFixture.timeout=true})
 await button.click();await expect(control.getByRole('alert')).toContainText('同一次操作')
 await page.evaluate(()=>{const f=(window as any).__resetFixture;f.timeout=false;f.outcome='alreadyRedeemed';f.count=0})
 await button.click();await expect(control).toContainText('0 张');await expect(button).toBeDisabled()
 const keys=await page.evaluate(()=>(window as any).__resetFixture.keys)
 expect(keys).toHaveLength(2);expect(keys[0]).toBe(keys[1])
})

test('home keeps five-hour and weekly quotas in separate full-width rows',async({page})=>{
 await page.setViewportSize({width:1200,height:800})
 await page.addInitScript(()=>{
  const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke
  g.__TAURI_INTERNALS__.invoke=async(command:string,args:any={})=>{
   const r=await invoke(command,args)
   if(command==='refresh_usage'&&args.id==='codex')return {...r,windows:[...r.windows,{...r.windows[0],label:'weekly',duration_minutes:10080,used_percent:42}]}
   if(command==='get_usage_refresh')return r.map((s:any)=>s.provider==='codex'?{...s,report:{...s.report,windows:[...s.report.windows,{...s.report.windows[0],label:'weekly',duration_minutes:10080,used_percent:42}]}}:s)
   return r
  }
 })
 await page.goto('http://127.0.0.1:1420/#/')
 await expect(page.locator('.usage-card.codex .usage-window')).toHaveCount(2)
 await page.evaluate(()=>document.fonts.ready)
 const windows=page.locator('.usage-card.codex .usage-window')
 const first=(await windows.nth(0).boundingBox())!,second=(await windows.nth(1).boundingBox())!,body=(await page.locator('.usage-card.codex .quota-windows').boundingBox())!
 expect(second.y).toBeGreaterThanOrEqual(first.y+first.height)
 expect(first.x).toBeCloseTo(second.x,0);expect(first.width).toBeCloseTo(body.width,0);expect(second.width).toBeCloseTo(body.width,0)
 expect(await page.locator('.usage-card.codex .progress').first().evaluate(el=>el.clientWidth)).toBeGreaterThan(350)
 const height=await page.locator('.workspace').evaluate(el=>({scroll:el.scrollHeight,visible:el.clientHeight}))
 expect(height.scroll).toBeLessThanOrEqual(height.visible)
 await page.screenshot({path:'../docs/screenshots/reset-card-layout/home-two-windows.png',fullPage:true})
})
