import {language,tr} from '../composables/i18n'
import { invoke, isTauri } from '@tauri-apps/api/core'
import type {TokenReport,ResetOutcome} from '../types'
import type { UsageRefreshState, Conversation, ConversationDetail, Monitoring, CreateTaskRequest, ProviderHealth, ProviderId, RunRecord, SessionPage, Settings, Task, UsageReport } from '../types'
export const desktop = isTauri()
export async function call<T>(command:string,args?:Record<string,unknown>):Promise<T> {
  if (!desktop) throw new Error('桌面服务未连接。请使用 npm run desktop 启动 Windows 应用。')
  return invoke<T>(command,args)
}
export const api={
  sessionLocks:(conversations:Pick<Conversation,'provider'|'session_id'>[])=>call<Record<string,import('../types').SessionLockState>>('get_session_lock_states',{conversations:conversations.map(({provider,session_id})=>({provider,session_id}))}),
  tokens:(days=30)=>call<TokenReport>('get_token_usage',{days,timezone:Intl.DateTimeFormat().resolvedOptions().timeZone}),
  monitoring:(days=7)=>call<Monitoring>('get_monitoring',{days}),
  defaults:()=>call<Settings>('get_default_settings'),
  detail:(conversation:Conversation)=>call<ConversationDetail>('get_conversation_detail',{conversation:JSON.parse(JSON.stringify(conversation))}),
  detect:(id:ProviderId)=>call<ProviderHealth>('detect_provider',{id}),
  providers:()=>call<ProviderHealth[]>('list_providers'),
  usage:(id:ProviderId)=>call<UsageReport>('refresh_usage',{id}),
  usageRefresh:()=>call<UsageRefreshState[]>('get_usage_refresh'),
  consumeResetCredit:(idempotencyKey:string,accountKey:string)=>call<ResetOutcome>('consume_reset_credit',{idempotencyKey,accountKey}),
  sessions:(provider:ProviderId,search:string,cursor:string|null=null)=>call<SessionPage>('list_conversations',{provider,search,cursor,limit:30}),
  tasks:()=>call<Task[]>('list_tasks'),
  runs:()=>call<RunRecord[]>('list_run_history'),
  settings:()=>call<Settings>('get_settings'),
  saveSettings:(settings:Settings)=>call<Settings>('update_settings',{settings:JSON.parse(JSON.stringify(settings))}),
  saveTask:(request:CreateTaskRequest,id?:string)=>call<Task>(id?'update_task':'create_task',{request:JSON.parse(JSON.stringify(request)),...(id?{id}:{})}),
  cancel:(id:string)=>call<void>('cancel_task',{id}),
  pause:(id:string,paused:boolean)=>call<Task>('pause_task',{id,paused}),
  run:(id:string)=>call<void>('run_task_now',{id}),
  log:(runId:string,offset=0)=>call<{text:string;next_offset:number;total_bytes:number}>('get_run_log',{runId,offset,limit:65536}),
}
export function providerName(id:ProviderId){return id==='codex'?'Codex':'Claude Code'}
export function canSchedule(c:Conversation|null){return !!c?.resumable&&(c.provider!=='codex'||c.lock_state!=='unknown')}
export function localInput(date=new Date(Date.now()+3600000)){return new Date(date.getTime()-date.getTimezoneOffset()*60000).toISOString().slice(0,16)}
export function formatDate(value:string|null,locale=language.value){if(!value)return '—';const date=new Date(value);return Number.isNaN(date.getTime())?'—':new Intl.DateTimeFormat(locale,{month:'short',day:'numeric',hour:'2-digit',minute:'2-digit'}).format(date)}
export function usageText(value:number|null){return value===null || !Number.isFinite(value) || value<0 || value>100?tr('未知'):`${Math.round(value)}%`}
export function remainingPercent(used:number|null):number|null {
  return used===null || !Number.isFinite(used) || used<0 || used>100 ? null : 100-used
}
export function windowLabel(minutes:number|null,fallback:string,locale=language.value):string {
  if(minutes===null || !Number.isInteger(minutes) || minutes<=0)return fallback
  const english=locale==='en-US'
  if(minutes%10080===0)return english?`${minutes/10080}-week window`:minutes===10080?'一周窗口':`${minutes/10080} 周窗口`
  if(minutes%1440===0)return english?`${minutes/1440}-day window`:`${minutes/1440} 天窗口`
  if(minutes%60===0)return english?`${minutes/60}-hour window`:minutes===300?'五小时窗口':`${minutes/60} 小时窗口`
  if(minutes>60)return english?`${Math.floor(minutes/60)}h ${minutes%60}m window`:`${Math.floor(minutes/60)} 小时 ${minutes%60} 分钟窗口`
  return english?`${minutes}-minute window`:`${minutes} 分钟窗口`
}
