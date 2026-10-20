import {defaultPretrigger} from '../services/pretrigger'
import {reactive} from 'vue'
import {api,desktop} from '../services/api'
import {language} from '../composables/i18n'
import type {TokenReport} from '../types'
import type {ProviderHealth,ProviderId,UsageReport,UsageRefreshState,Task,RunRecord,Settings,Monitoring} from '../types'
export const defaultSettings:Settings={language:'zh-CN',theme:'indigo',close_to_tray:true,autostart:false,paused:false,notifications:true,reminder_minutes:10,pretrigger_enabled:true,pretrigger:structuredClone(defaultPretrigger),authorized_roots:[],save_output:false,startup_minimized:false,default_time:'06:00',default_wake:false,default_catch_up:false,notify_start:false,notify_success:true,notify_failure:true,notification_sound:true,connection_interval_minutes:10,appearance:'system',paused_providers:[]}
export const store=reactive({providers:[] as ProviderHealth[],usage:{} as Partial<Record<ProviderId,UsageReport>>,usageRefresh:{} as Partial<Record<ProviderId,UsageRefreshState>>,tasks:[] as Task[],runs:[] as RunRecord[],settings:null as Settings|null,monitoring:{snapshots:[],notifications:[]} as Monitoring,resetting:false,busy:false,error:'',notice:'',noticeTopic:'',errorTopic:'',now:Date.now(),startedAt:Date.now()})
export function applySettings(settings:Settings){language.value=settings.language;document.documentElement.lang=settings.language;document.documentElement.dataset.theme=settings.theme;document.documentElement.dataset.appearance=settings.appearance||'system'}
export const tokenStore=reactive({report:null as TokenReport|null,busy:false,error:''})
let tokenRequest:Promise<void>|undefined,lastTokenRead=0
export function loadTokens(force=false):Promise<void>{
 if(!desktop)return Promise.resolve()
 if(tokenRequest)return tokenRequest
 if(!force&&Date.now()-lastTokenRead<15000)return Promise.resolve()
 tokenStore.busy=true;tokenStore.error=''
 tokenRequest=api.tokens(30).then(report=>{tokenStore.report=report;lastTokenRead=Date.now()}).catch(e=>{tokenStore.error=String(e)}).finally(()=>{tokenStore.busy=false;tokenRequest=undefined})
 return tokenRequest
}
export function applyUsageRefresh(state:UsageRefreshState){store.usageRefresh[state.provider]=state;if(state.report)store.usage[state.provider]=state.report}
export async function loadUsageRefresh(){if(desktop)for(const state of await api.usageRefresh())applyUsageRefresh(state)}
export async function loadLocal(){if(!desktop)return;try{const[tasks,runs,settings]=await Promise.all([api.tasks(),api.runs(),api.settings(),loadUsageRefresh()]);store.tasks=tasks;store.runs=runs;store.settings=settings;applySettings(settings)}catch(e){store.error=String(e)}}
export async function loadMonitoring(){if(desktop)store.monitoring=await api.monitoring(30)}
let refreshRequest:Promise<void>|undefined
export function refresh():Promise<void>{
 if(!desktop||store.resetting)return Promise.resolve()
 if(refreshRequest)return refreshRequest
 store.busy=true;store.error=''
 refreshRequest=(async()=>{try{await loadLocal();store.providers=await api.providers();await Promise.all([api.usage('codex'),api.usage('claude_code')]);await Promise.all([loadUsageRefresh(),loadMonitoring()])}catch(e){store.error=String(e)}finally{store.busy=false;refreshRequest=undefined}})()
 return refreshRequest
}
export async function action(work:()=>Promise<unknown>,message='操作已完成',topic=''){store.error='';try{await work();store.noticeTopic=topic;store.notice=message;await loadLocal()}catch(e){store.errorTopic=topic;store.error=String(e)}}
export function providerEnabled(id:ProviderId){return !!store.settings&&!store.settings.paused&&!store.settings.paused_providers?.includes(id)}
export async function toggleProvider(id:ProviderId,enabled:boolean){if(!store.settings)return;const paused=new Set(store.settings.paused_providers||[]);if(enabled)paused.delete(id);else paused.add(id);await action(()=>api.saveSettings({...store.settings!,paused_providers:[...paused]}),enabled?'已恢复该工具的后续调度':'已暂停该工具的后续调度',`provider:${id}:scheduling`)}
