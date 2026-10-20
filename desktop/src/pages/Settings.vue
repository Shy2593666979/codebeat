<script setup lang="ts">
import {tr} from '../composables/i18n'
import {computed,onBeforeUnmount,onMounted,ref,watch} from 'vue'
import {Rocket,Bell,Clock3,RotateCw,Database} from 'lucide-vue-next'
import PageHeader from '../components/PageHeader.vue'
import ProviderIcon from '../components/ProviderIcon.vue'
import ToggleSwitch from '../components/ToggleSwitch.vue'
import Select from '../components/Select.vue'
import PretriggerSettings from '../components/PretriggerSettings.vue'
import {store,defaultSettings} from '../stores/store'
import {api,call,desktop,providerName} from '../services/api'
import {parseSettingsFile} from '../services/settings-file'
import {queueSettingsPatch,flushSettings,settingsSaveState} from '../stores/settings-autosave'
import type {Settings,ProviderId} from '../types'

const displayScale=ref(0),displayScaleBusy=ref(false)
const displayScaleOptions=computed(()=>[{value:0,label:tr('按屏幕适配')},...[0.8,0.9,1,1.1,1.25].map(value=>({value,label:Math.round(value*100)+'%'}))])
onMounted(async()=>{if(desktop)try{displayScale.value=(await call<{selected:number}>('get_display_layout')).selected}catch{}})
async function changeDisplayScale(value:number){displayScaleBusy.value=true;try{const result=await call<{selected:number}>('set_display_scale',{value});displayScale.value=result.selected}catch(e){localError.value=String(e)}finally{displayScaleBusy.value=false}}
const appearanceOptions=computed(()=>[{value:'system',label:tr("跟随 Windows")},{value:'light',label:tr("浅色模式")},{value:'dark',label:tr("深色模式")}] as const)
const languageOptions=computed(()=>[{value:'zh-CN',label:tr("中文（简体）")},{value:'en-US',label:'English'}] as const)
const reminderOptions=computed(()=>[{value:0,label:tr("关闭")},{value:5,label:tr("5 分钟")},{value:10,label:tr("10 分钟")},{value:30,label:tr("30 分钟")},{value:60,label:tr("60 分钟")}] as const)
const connectionIntervalOptions=computed(()=>[{value:5,label:tr("每 5 分钟")},{value:10,label:tr("每 10 分钟")},{value:30,label:tr("每 30 分钟")},{value:60,label:tr("每 60 分钟")}] as const)
const clone=<T>(v:T):T=>JSON.parse(JSON.stringify(v))
const draft=ref<Settings>(clone(store.settings||defaultSettings)),localError=ref(''),testing=ref<ProviderId>(),operationStatus=ref('')
let known=clone(draft.value),syncing=false
const edited=new Set<keyof Settings>()
watch(draft,value=>{
 if(syncing)return
 for(const key of Object.keys(value) as (keyof Settings)[])if(JSON.stringify(value[key])!==JSON.stringify(known[key]))edited.add(key)
 known=clone(value)
 try{parseSettingsFile(JSON.stringify(value));localError.value=''}catch(e){localError.value=String(e).replace(/^Error: /,'');return}
 const patch:Partial<Settings>={};for(const key of edited)Object.assign(patch,{[key]:clone(value[key])})
 if(desktop&&edited.size)queueSettingsPatch(patch)
},{deep:true,flush:'sync'})
watch([()=>store.settings,()=>settingsSaveState.saving,()=>settingsSaveState.pending],([settings])=>{
 if(!settings)return
 syncing=true
 const next=clone(draft.value)
 for(const key of Object.keys(settings) as (keyof Settings)[]){
  if(edited.has(key)){
   if(!settingsSaveState.saving&&!settingsSaveState.pending&&JSON.stringify(settings[key])===JSON.stringify(next[key]))edited.delete(key)
  }else Object.assign(next,{[key]:clone(settings[key])})
 }
 draft.value=next;known=clone(next);syncing=false
},{deep:true})
onBeforeUnmount(()=>{if(desktop)void flushSettings()})
const status=computed(()=>localError.value||settingsSaveState.error)
const providerStatus=(id:ProviderId)=>{const h=store.providers.find(p=>p.id===id);return h?.status==='ready'?'已连接':h?.status==='missing'?'未安装':'待检测'}
async function detect(id:ProviderId){testing.value=id;try{const health=await api.detect(id);const index=store.providers.findIndex(p=>p.id===id);if(index>=0)store.providers[index]=health;else store.providers.push(health);operationStatus.value=providerName(id)+': '+tr(health.message)}catch(e){operationStatus.value=String(e)}finally{testing.value=undefined}}
</script>
<template><div class="settings-page">
 <PageHeader :title="tr('设置')" :subtitle="tr('按你的习惯安排工作')"><span v-if="status" class="settings-auto-state" :class="{error:localError||settingsSaveState.error}" role="status">{{tr(status)}}</span><button v-if="settingsSaveState.error" class="text-button" @click="flushSettings">{{tr("重试")}}</button></PageHeader>
 <div class="settings-board">
  <div class="settings-column">
   <section class="panel settings-card"><h2><Rocket/>{{tr("启动与外观")}}</h2>
    <label class="setting-row"><span><strong>{{tr("开机自动启动")}}</strong></span><ToggleSwitch :label="tr('开机自动启动')" v-model="draft.autostart"/></label>
    <label class="setting-row"><span><strong>{{tr("启动后最小化到托盘")}}</strong></span><ToggleSwitch :label="tr('启动后最小化到系统托盘')" v-model="draft.startup_minimized"/></label>
    <label class="setting-row"><span><strong>{{tr("关闭窗口时留在托盘")}}</strong></span><ToggleSwitch :label="tr('关闭窗口时最小化到托盘')" v-model="draft.close_to_tray"/></label>
    <div class="setting-row"><span><strong>{{tr("系统主题")}}</strong></span><Select v-model="draft.appearance" :options="appearanceOptions" :aria-label="tr('系统主题')"/></div>
    <div class="setting-row"><span><strong>{{tr("界面语言")}}</strong></span><Select v-model="draft.language" :options="languageOptions" :aria-label="tr('界面语言')"/></div>
    <div class="setting-row"><span><strong>{{tr('界面缩放')}}</strong></span><Select :model-value="displayScale" :options="displayScaleOptions" :disabled="displayScaleBusy||!desktop" :aria-label="tr('界面缩放')" @update:model-value="changeDisplayScale"/></div>
   </section>
   <section class="panel settings-card"><h2><Clock3/>{{tr("任务默认设置")}}</h2>
    <label class="setting-row"><span><strong>{{tr("默认触发时间")}}</strong></span><input type="time" v-model="draft.default_time" :aria-label="tr('默认触发时间')"></label>
    <div class="setting-row"><span><strong>{{tr("提醒提前")}}</strong></span><Select v-model="draft.reminder_minutes" :options="reminderOptions" :aria-label="tr('默认提醒提前')"/></div>
    <label class="setting-row"><span><strong>{{tr("允许唤醒电脑")}}</strong></span><ToggleSwitch :label="tr('允许唤醒电脑执行任务')" v-model="draft.default_wake"/></label>
    <label class="setting-row"><span><strong>{{tr("错过后补执行")}}</strong></span><ToggleSwitch :label="tr('错过任务后补执行')" v-model="draft.default_catch_up"/></label>
    <label class="setting-row"><span><strong>{{tr("自动继续")}}</strong></span><ToggleSwitch :label="tr('默认自动继续')" :model-value="!draft.paused" @update:model-value="v=>draft.paused=!v"/></label>
   </section>
  </div>
  <div class="settings-column">
   <section class="panel settings-card"><h2><Bell/>{{tr("消息通知")}}</h2>
    <label class="setting-row"><span><strong>{{tr("显示桌面通知")}}</strong></span><ToggleSwitch :label="tr('显示桌面通知')" v-model="draft.notifications"/></label>
    <label class="setting-row"><span><strong>{{tr("播放提示音")}}</strong></span><ToggleSwitch :label="tr('播放提示音')" v-model="draft.notification_sound"/></label>
    <label class="setting-row"><span><strong>{{tr("任务开始时通知")}}</strong></span><ToggleSwitch :label="tr('任务开始时通知')" v-model="draft.notify_start"/></label>
    <label class="setting-row"><span><strong>{{tr("任务完成时通知")}}</strong></span><ToggleSwitch :label="tr('任务完成时通知')" v-model="draft.notify_success"/></label>
    <label class="setting-row"><span><strong>{{tr("任务失败时通知")}}</strong></span><ToggleSwitch :label="tr('任务失败时通知')" v-model="draft.notify_failure"/></label>
   </section>
   <section class="panel settings-card provider-settings"><h2><Database/>{{tr("工具连接")}}</h2>
    <article v-for="id in (['codex','claude_code'] as const)" :key="id" class="provider-settings-row"><ProviderIcon :provider="id"/><div><strong>{{providerName(id)}}</strong><span :class="{connected:providerStatus(id)==='已连接'}"><i class="status-dot" :class="{offline:providerStatus(id)!=='已连接'}"/>{{tr(providerStatus(id))}}</span></div><button class="icon-button" :aria-label="tr('重新检测 ')+providerName(id)" :disabled="!!testing||!desktop" @click="detect(id)"><RotateCw :size="16" :class="{spin:testing===id}"/></button></article>
    <div class="setting-row"><span><strong>{{tr("检测间隔")}}</strong></span><Select v-model="draft.connection_interval_minutes" :options="connectionIntervalOptions" :aria-label="tr('自动检测连接状态')"/></div>
   </section>
  </div>
  <PretriggerSettings v-model="draft.pretrigger" v-model:enabled="draft.pretrigger_enabled" :paused="draft.paused"/>
 </div>
 <p v-if="operationStatus" class="settings-operation" role="status">{{tr(operationStatus)}}</p>
</div></template>
<style scoped>
.settings-page .page-heading{min-height:59px;margin-bottom:14px}.settings-page :deep(.page-heading h1){font-size:28px}.settings-page :deep(.page-heading p){font-size:13px;margin-top:4px}.settings-auto-state{font-size:12px;color:var(--muted);max-width:360px;line-height:1.6}.settings-auto-state.error{color:var(--danger)}.settings-board{grid-template-columns:minmax(0,1fr) minmax(0,1fr) minmax(0,1.5fr);gap:18px}.settings-column{gap:18px;min-width:0}.settings-card{padding:17px 18px 11px;min-width:0}.settings-card>h2{font-size:17px;gap:9px;padding-bottom:12px;margin-bottom:5px}.settings-card>h2>svg{width:20px;height:20px}.settings-page .settings-column .settings-card .setting-row{min-height:41px;padding:4px 0;gap:10px}.setting-row strong{font-size:13px;line-height:1.5}.setting-row :deep(.select-control),.setting-row input[type=time]{height:33px;min-height:33px;font-size:12px;padding:4px 8px}.setting-row :deep(.select-wrapper){width:123px;flex-shrink:0}.setting-row input[type=time]{width:123px;max-width:123px}.setting-row :deep(.toggle-switch){transform:scale(1);transform-origin:right center}.provider-settings-row{padding:11px;gap:10px;margin:11px 0}.provider-settings-row :deep(.provider-mark){width:31px;height:31px;border-radius:8px}.provider-settings-row strong{font-size:12px;margin-bottom:2px}.provider-settings-row span{font-size:10px}.provider-settings-row .icon-button{min-height:28px;width:28px;padding:5px}.settings-operation{font-size:11px;color:var(--muted);margin:8px 0}
@media(max-width:1100px){.settings-page .settings-column .settings-card .setting-row{min-height:36px;padding:2px 0;gap:8px}.settings-column{gap:12px}.settings-card{padding-top:13px;padding-bottom:7px}.settings-card>h2{padding-bottom:10px;margin-bottom:3px}.provider-settings-row{padding:8px;margin:8px 0}.setting-row :deep(.select-control),.setting-row input[type=time]{height:31px;min-height:31px}.settings-card{padding-left:10px;padding-right:10px}.settings-board{gap:10px;grid-template-columns:minmax(0,1fr) minmax(0,1fr) minmax(0,1.45fr)}.setting-row :deep(.select-wrapper),.setting-row input[type=time]{width:110px;max-width:110px}.setting-row strong{font-size:11px}.settings-card>h2{font-size:14px}}
@media(max-width:850px){.settings-board{grid-template-columns:1fr 1fr}.settings-board>.pretrigger-settings{grid-column:1/-1}.settings-page .page-heading{min-height:59px}.settings-auto-state{max-width:180px}.settings-page :deep(.heading-actions){max-width:45%}}
@media(max-width:580px){.settings-board{grid-template-columns:1fr}.settings-card>h2{font-size:16px}.setting-row strong{font-size:13px}.setting-row :deep(.select-wrapper),.setting-row input[type=time]{width:145px;max-width:145px}}
</style>
