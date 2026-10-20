import {reactive} from 'vue'
import {api} from './api'
import {store,loadUsageRefresh} from '../stores/store'
import {tr} from '../composables/i18n'
import type {ResetOutcome} from '../types'

export const resetState=reactive({busy:false,message:'',error:''})
const messages:Record<ResetOutcome,string>={
 reset:'重置卡已使用，额度已重新读取',
 alreadyRedeemed:'该次重置已完成，额度已重新读取',
 nothingToReset:'当前没有可重置的额度窗口',
 noCredit:'当前没有可用的重置卡',
}

/** 先持久化再发送。页面重载和超时必须重试同一次兑换。 */
export function redemptionAttempt(storage:Pick<Storage,'getItem'|'setItem'>,account:string):string{
 const key='codebeat.reset-attempt.'+account
 const previous=storage.getItem(key)
 if(previous){
  if(!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(previous))throw new Error('RESET_INVALID_ATTEMPT')
  return previous
 }
 const attempt=crypto.randomUUID()
 storage.setItem(key,attempt)
 return attempt
}

export async function useResetCredit(){
 const credits=store.usage.codex?.reset_credits
 if(resetState.busy||store.busy||store.usageRefresh.codex?.retrying||!credits?.account_key||credits.available_count<1)return
 resetState.busy=true;store.resetting=true;resetState.message='';resetState.error=''
 const account=credits.account_key
 let outcome:ResetOutcome|undefined
 try{
  const key=redemptionAttempt(localStorage,account)
  outcome=await api.consumeResetCredit(key,account)
  if(!Object.hasOwn(messages,outcome))throw new Error('RESET_UNKNOWN_OUTCOME')
  // 只有在收到明确且可识别的服务端结果后才清除。所有不确定的失败
  // 都保留这次尝试，包括 webview/应用重启之后。
  // 立即作废旧计数，避免刷新失败时仍展示过期的卡片。
  if(store.usage.codex)store.usage.codex.reset_credits=null
  localStorage.removeItem('codebeat.reset-attempt.'+account)
  const report=await api.usage('codex')
  if(report.status==='available')store.usage.codex=report
  else await loadUsageRefresh()
  if(report.status==='unavailable'){
   resetState.message=tr(outcome==='reset'||outcome==='alreadyRedeemed'?'重置已完成，额度暂未读取成功，请刷新':'操作已完成，额度暂未读取成功，请刷新')
  }else resetState.message=tr(messages[outcome])
 }catch(e){
  if(outcome&&Object.hasOwn(messages,outcome)){
   resetState.message=tr(outcome==='reset'||outcome==='alreadyRedeemed'?'重置已完成，额度暂未读取成功，请刷新':'操作已完成，额度暂未读取成功，请刷新')
  }else{
   resetState.error=String(e).includes('RESET_ACCOUNT_CHANGED')?tr('登录账号已变化，请刷新额度后再试'):tr('操作结果尚未确认，再次点击将重试同一次操作')
  }
 }finally{resetState.busy=false;store.resetting=false}
}
