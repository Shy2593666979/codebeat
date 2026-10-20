<script setup lang="ts">
import {tr} from '../composables/i18n'
import {computed,ref} from 'vue'
import {Plus,Clock3,Monitor,Database} from 'lucide-vue-next'
import UsageCard from '../components/UsageCard.vue'
import UsageTrend from '../components/UsageTrend.vue'
import TaskModal from '../components/TaskModal.vue'
import {store} from '../stores/store'
import {durationText} from '../services/presentation'
const editing=ref(false)
const running=computed(()=>store.tasks.filter(t=>['running','claimed'].includes(t.state)).length)
</script>
<template><div class="page-heading home-heading"><div><h1>{{tr("欢迎使用 Code")}}<span class="brand-accent">Beat</span></h1><p>{{tr("让 AI 编程的时间，更有节奏")}}</p></div><button class="primary new-button" @click="editing=true"><Plus/>{{tr("新建任务")}}</button></div><div class="usage-grid"><UsageCard provider="codex" compact/><UsageCard provider="claude_code" compact/></div>
<div class="home-bottom"><section class="panel today-panel"><div class="panel-heading"><h2>{{tr("今日状态")}}</h2><span class="live-label"><i class="status-dot" :class="{offline:!store.settings||store.settings.paused}"/>{{!store.settings?tr("桌面服务未连接"):store.settings.paused?tr("调度已暂停"):tr("后台运行中")}}</span></div><div class="today-stats"><div><span class="stat-icon blue"><Monitor/></span><small>{{tr("当前状态")}}</small><strong>{{!store.settings?tr("尚未连接"):store.settings.paused?tr("监控已暂停"):tr("监控运行中")}}</strong><p>{{store.settings?tr("正在监控任务和额度"):tr("请启动 Windows 桌面应用")}}</p></div><div><span class="stat-icon purple"><Database/></span><small>{{tr("运行任务")}}</small><strong>{{running}}</strong><p>{{tr("个任务在后台运行")}}</p></div><div><span class="stat-icon blue"><Clock3/></span><small>{{tr("本次运行时长")}}</small><strong>{{durationText(store.now-store.startedAt)}}</strong><p>{{tr("自本次启动起")}}</p></div></div></section><UsageTrend compact/></div><TaskModal v-if="editing" @close="editing=false" @saved="editing=false"/></template>

<style scoped>
.home-bottom{grid-template-columns:repeat(2,minmax(0,1fr));gap:15px;margin-top:20px;align-items:stretch}
.home-bottom> :deep(.panel),.today-panel{min-width:0}
.today-panel{display:flex;flex-direction:column;min-height:310px}
.home-bottom :deep(.panel-heading){padding:18px 20px}
.today-stats{flex:1;align-content:center;padding:14px 20px 30px}
.today-stats .stat-icon{width:52px;height:52px;margin-bottom:12px}
.today-stats strong{font-size:21px;margin:6px 0}
.today-stats small{font-size:13px}
.today-stats p{font-size:11px;white-space:normal;line-height:1.6}
@media(max-width:1240px){.today-stats{padding-left:16px;padding-right:16px}.today-stats>div+div{padding-left:12px}.today-stats strong{font-size:18px}.today-stats small{font-size:12px}}
@media(max-width:980px){.home-bottom{grid-template-columns:repeat(2,minmax(0,1fr))}.today-panel{min-height:260px}.today-stats{padding-top:16px;padding-bottom:24px}}
@media(max-width:640px){.home-bottom{grid-template-columns:1fr;gap:12px}.home-bottom :deep(.panel-heading){padding-left:14px;padding-right:14px}.today-stats strong{font-size:15px}.today-stats small{font-size:11px}.today-stats p{font-size:10px}.today-stats .stat-icon{width:44px;height:44px}}
</style>
