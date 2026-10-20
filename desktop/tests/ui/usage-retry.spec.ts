import {test,expect} from '@playwright/test'
import path from 'node:path'
import {installDesktopFixture} from './fixtures'
test.use({viewport:{width:1200,height:800},locale:'zh-CN'})
test.beforeEach(async({page})=>{
 await page.route('http://127.0.0.1:1420/**',async route=>{const n=new URL(route.request().url()).pathname;await route.fulfill({path:path.resolve('dist',n==='/'?'index.html':n.slice(1))})})
 await installDesktopFixture(page)
 await page.addInitScript(()=>{
  const g=window as any,invoke=g.__TAURI_INTERNALS__.invoke
  g.__TAURI_INTERNALS__.invoke=async(command:string,args:any)=>{
   const result=await invoke(command,args)
   if(command==='get_usage_refresh'&&!g.__usageRefresh){g.__usageRefresh=result;result.find((s:any)=>s.provider==='codex').report.reset_credits={available_count:2,account_key:'fixture-account'}}
   if(command==='refresh_usage'&&args.id==='codex'&&g.__quotaFail){
    const states=await invoke('get_usage_refresh'),s=states.find((s:any)=>s.provider==='codex')
    Object.assign(s,{retrying:true,refreshing:false,last_error:'RPC_ERROR: network',next_retry_at:new Date(Date.now()+5000).toISOString()})
    if(g.__noQuotaCache)s.report={provider:'codex',status:'unavailable',windows:[],message:'RPC_ERROR: network'}
    g.__usageRefresh=states;g.__codebeatEmit('usage-updated',s)
    return {provider:'codex',status:'unavailable',windows:[],message:'RPC_ERROR: network'}
   }
   return result
  }
 })
})
test('a failed refresh retains quota and reset time; background success replaces it without a popup',async({page})=>{
 await page.goto('http://127.0.0.1:1420/#/usage')
 const card=page.locator('.usage-card.codex'),bars=card.getByRole('progressbar')
 await expect(bars.first()).toBeVisible()
 await expect(card.getByRole('button',{name:'使用重置卡',exact:true})).toBeEnabled()
 const before=await bars.evaluateAll(els=>els.map(el=>el.getAttribute('aria-valuenow')))
 const reset=await card.locator('.summary-next').innerText()
 await page.evaluate(()=>(window as any).__quotaFail=true)
 await page.getByRole('button',{name:'刷新额度',exact:true}).click()
 await expect(card).toContainText('上次成功数据 · 自动重试中')
 expect(await bars.evaluateAll(els=>els.map(el=>el.getAttribute('aria-valuenow')))).toEqual(before)
 expect(await card.locator('.summary-next').innerText()).toBe(reset)
 await expect(page.locator('.feedback-card')).toHaveCount(0)
 await expect(card.getByRole('button',{name:'使用重置卡',exact:true})).toBeDisabled()
 await page.evaluate(()=>{const g=window as any,s=g.__usageRefresh.find((s:any)=>s.provider==='codex');s.report.windows.forEach((w:any)=>w.used_percent=42);Object.assign(s,{retrying:false,last_error:null,next_retry_at:null});g.__codebeatEmit('usage-updated',s)})
 await expect(bars.first()).toHaveAttribute('aria-valuenow','58')
 await expect(card).not.toContainText('自动重试中')
 await expect(page.locator('.feedback-card')).toHaveCount(0)
 await page.screenshot({path:'../docs/screenshots/usage-retry/recovered.png',animations:'disabled'})
})
test('without any successful cache, retry state is clear and does not print the raw RPC error',async({page})=>{
 await page.addInitScript(()=>{const g=window as any;g.__quotaFail=true;g.__noQuotaCache=true})
 await page.goto('http://127.0.0.1:1420/#/')
 const card=page.locator('.usage-card.codex')
 await expect(card).toContainText('读取失败 · 自动重试中')
 await expect(card).toContainText('读取失败，正在自动重试')
 await expect(card).not.toContainText('RPC_ERROR')
 await expect(card.getByRole('progressbar')).toHaveCount(0)
 await expect(page.locator('.feedback-card')).toHaveCount(0)
 await page.screenshot({path:'../docs/screenshots/usage-retry/no-cache.png',animations:'disabled'})
})
