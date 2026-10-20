<script setup lang="ts">
import {tr,language} from '../composables/i18n'
import {computed} from 'vue'
import {Search,Play,Pause,Bell,Clock3,RotateCw,ArrowRight,Hourglass} from 'lucide-vue-next'
import PageHeader from '../components/PageHeader.vue'
import UsageCard from '../components/UsageCard.vue'
import UsageTrend from '../components/UsageTrend.vue'
import ProviderIcon from '../components/ProviderIcon.vue'
import {store,refresh} from '../stores/store'
import {formatDate,providerName} from '../services/api'
import {relativeTime} from '../services/presentation'
import {stateLabel} from '../types/states'
import type {ProviderId} from '../types'
const resets=computed(()=>Object.values(store.usage).flatMap(r=>r?.windows.filter(w=>w.resets_at_utc).map(w=>({provider:r.provider,...w}))||[]).sort((a,b)=>a.resets_at_utc!.localeCompare(b.resets_at_utc!)))
const today=(value:string)=>new Date(value).toDateString()===new Date(store.now).toDateString()
const checks=computed(()=>store.monitoring.snapshots.filter(s=>today(s.observed_at)).length)
const successes=computed(()=>store.runs.filter(r=>r.state==='succeeded'&&today(r.finished_at||r.started_at)).length)
const paused=computed(()=>store.tasks.filter(t=>t.state==='paused').length)
const notifications=computed(()=>store.monitoring.notifications.filter(n=>today(n.at)).length)
const events=computed(()=>[...store.monitoring.snapshots.map(s=>({key:s.observed_at+s.report.provider,at:s.observed_at,provider:s.report.provider,event:s.report.status==='available'?'额度读取成功':'额度暂不可用',detail:s.report.message,state:s.report.status==='available'?'success':'warning'})),...store.runs.map(r=>({key:r.id,at:r.finished_at||r.started_at,provider:store.tasks.find(t=>t.id===r.task_id)?.request.conversation.provider,event:stateLabel(r.state),detail:r.error_message||store.tasks.find(t=>t.id===r.task_id)?.request.title||r.task_id,state:r.state==='succeeded'?'success':'warning'}))].sort((a,b)=>b.at.localeCompare(a.at)).slice(0,20))
</script>
<template><PageHeader :title="tr('额度监控')" :subtitle="tr('实时查看剩余额度，按你的计划继续工作')"><button class="icon-button refresh-usage" :disabled="store.busy" :title="tr('刷新额度')" @click="refresh"><RotateCw :class="{spin:store.busy}"/></button></PageHeader><div class="usage-grid"><UsageCard provider="codex"/><UsageCard provider="claude_code"/></div><div class="usage-middle"><UsageTrend/><section class="panel reset-panel"><div class="panel-heading"><h2>{{tr("下次重置时间")}}</h2><RouterLink class="text-link" to="/tasks">{{tr("查看任务")}} <ArrowRight :size="17"/></RouterLink></div><div class="reset-timeline"><div v-for="window in resets" :key="window.provider+window.label"><span class="timeline-icon" :class="window.provider"><Clock3/></span><span><strong>{{providerName(window.provider)}} {{tr("重置：")}}{{new Date(window.resets_at_utc!).toLocaleTimeString(language,{hour:'2-digit',minute:'2-digit'})}}</strong><small>{{relativeTime(window.resets_at_utc,store.now)}}</small></span><time>{{formatDate(window.resets_at_utc)}}</time></div><div v-if="!resets.length" class="timeline-empty"><Hourglass/><span>{{tr("暂无已确认的重置时间")}}<small>{{tr("读取官方额度窗口后在这里显示")}}</small></span></div><div><span class="timeline-icon"><Clock3/></span><span><strong>{{tr("后台定时检查")}}</strong><small>{{tr("在设置中启用维护入口后，每 5 分钟检查")}}</small></span><RouterLink to="/settings">{{tr("设置")}}</RouterLink></div></div></section></div><div class="monitor-stats"><article class="panel"><span class="stat-icon blue"><Search/></span><div><small>{{tr("今日检查次数")}}</small><strong>{{checks}} <span>{{tr("次")}}</span></strong><p>{{tr("本地实际记录")}}</p></div></article><article class="panel"><span class="stat-icon teal"><Play/></span><div><small>{{tr("成功执行任务")}}</small><strong>{{successes}} <span>{{tr("个")}}</span></strong><p>{{tr("今日确认成功")}}</p></div></article><article class="panel"><span class="stat-icon purple"><Pause/></span><div><small>{{tr("暂停的任务")}}</small><strong>{{paused}} <span>{{tr("个")}}</span></strong><p>{{tr("当前暂停中")}}</p></div></article><article class="panel"><span class="stat-icon orange"><Bell/></span><div><small>{{tr("发送提醒")}}</small><strong>{{notifications}} <span>{{tr("次")}}</span></strong><p>{{tr("今日已发送")}}</p></div></article></div><section class="panel event-panel"><div class="panel-heading"><h2>{{tr("监控事件记录")}}</h2><RouterLink class="text-link" to="/history">{{tr("查看运行历史")}} <ArrowRight :size="17"/></RouterLink></div><div class="table-scroll"><table><thead><tr><th>{{tr("时间")}}</th><th>{{tr("服务商")}}</th><th>{{tr("事件")}}</th><th>{{tr("详情")}}</th></tr></thead><tbody><tr v-for="event in events" :key="event.key"><td>{{formatDate(event.at)}}</td><td><span class="table-provider"><ProviderIcon v-if="event.provider" :provider="event.provider" size="small"/>{{event.provider?providerName(event.provider):'—'}}</span></td><td><span class="event-name"><i class="status-dot" :class="{orange:event.state==='warning'}"/>{{tr(event.event)}}</span></td><td>{{tr(event.detail)}}</td></tr></tbody></table></div><p v-if="!events.length" class="empty-table">{{tr("暂无监控事件，读取额度或执行任务后会生成记录。")}}</p></section></template>

<style scoped>
.usage-grid,.usage-middle{grid-template-columns:repeat(2,minmax(0,1fr));gap:15px}
.usage-grid>* , .usage-middle>*{min-width:0}
@media(max-width:640px){.usage-grid,.usage-middle{grid-template-columns:minmax(0,1fr);gap:12px}}
</style>
