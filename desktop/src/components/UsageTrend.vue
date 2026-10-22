<script setup lang="ts">
import {tr,language} from '../composables/i18n'
import {computed,onMounted,ref,useId} from 'vue'
import {ChevronRight,Activity} from 'lucide-vue-next'
import {tokenStore,store,loadTokens} from '../stores/store'
import {desktop,providerName} from '../services/api'
import {tokenDays,tokenLabel,tokenTotal,tokenSeries,tokenCurve} from '../services/token-usage'
import type {ProviderId} from '../types'
import Select from './Select.vue'
import AppDialog from './AppDialog.vue'
import ProviderIcon from './ProviderIcon.vue'
const props=withDefaults(defineProps<{compact?:boolean}>(),{compact:false})
const days=ref(30),provider=ref<ProviderId|'all'>('all'),detail=ref(false),selectedDay=ref<string|null>(null)
const providerOptions=computed(()=>[{value:'all',label:tr("全部提供商")},{value:'codex',label:'Codex'},{value:'claude_code',label:'Claude Code'}] as const)
const daysOptions=computed(()=>[{value:7,label:tr("近 7 天")},{value:14,label:tr("近 14 天")},{value:30,label:tr("近 30 天")}] as const)
const chart=computed(()=>tokenDays(tokenStore.report,days.value,store.now))
const rows=computed(()=>chart.value.flatMap(d=>d.rows).filter(r=>provider.value==='all'||r.provider===provider.value))
const sources=computed(()=>tokenStore.report?.sources.filter(s=>provider.value==='all'||s.provider===provider.value)||[])
const knownProvider=(id:ProviderId)=>!!tokenStore.report?.sources.some(s=>s.provider===id&&s.records>0)
const known=computed(()=>sources.value.some(s=>s.records>0))
const hasData=computed(()=>rows.value.some(r=>r.requests>0))
const series=computed(()=>tokenSeries(chart.value,provider.value))
const maximum=computed(()=>Math.max(2,...series.value.map(day=>day.total)))
const scale=computed(()=>{const raw=maximum.value;const step=10**Math.floor(Math.log10(raw));return Math.ceil(raw/step)*step})
const detailRows=computed(()=>rows.value.filter(r=>!selectedDay.value||r.date===selectedDay.value).slice().sort((a,b)=>b.date.localeCompare(a.date)||a.provider.localeCompare(b.provider)))
const curve=computed(()=>tokenCurve(series.value.map(day=>day.total),scale.value))
const gradientId=useId()+'-token-area',activeDay=ref(-1)
const sourceText=(status:string)=>status==='missing'?'未找到本机用量日志':status==='empty'?'日志中暂无可识别的用量':status==='partial'?'部分记录尚未纳入':'已读取本机用量日志'
function inspect(date:string|null=null){selectedDay.value=date;detail.value=true}
onMounted(()=>{void loadTokens()})
</script>
<template>
 <section class="panel trend-panel token-trend" :aria-busy="tokenStore.busy">
  <div class="panel-heading"><h2><button class="token-chart-title" :title="tr('近 {days} 天 Token 消耗',{days})" @click="inspect()"><span>{{tr('近 {days} 天 Token 消耗',{days})}}</span><ChevronRight :size="16"/></button></h2><div class="token-controls"><Select v-model="provider" :options="providerOptions" :aria-label="tr('趋势提供商')"/><Select v-if="!compact" v-model="days" :options="daysOptions" :aria-label="tr('趋势时间范围')"/></div></div>
  <div class="token-curve-chart" :aria-label="tr('每日 Token 消耗曲线图')"><div class="token-curve-axis"><span>{{tokenLabel(scale)}}</span><span>{{tokenLabel(scale/2)}}</span><span>0</span></div><div class="token-curve-plot" @mouseleave="activeDay=-1"><svg viewBox="0 0 700 150" preserveAspectRatio="none" aria-hidden="true"><defs><linearGradient :id="gradientId" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stop-color="var(--primary)" stop-opacity=".18"/><stop offset="100%" stop-color="var(--primary)" stop-opacity=".01"/></linearGradient></defs><path v-for="y in [8,74,140]" :key="y" :d="'M 0 '+y+' H 700'" class="token-grid"/><template v-if="hasData"><path class="token-curve-area" :d="curve.area" :fill="'url(#'+gradientId+')'"/><path class="token-curve-line" :d="curve.line"/><circle v-for="(point,i) in curve.points" :key="series[i].key" :cx="point.x" :cy="point.y" :r="activeDay===i?4.5:2.5" class="token-curve-point"/></template></svg><div class="token-date-targets"><button v-for="(day,i) in series" :key="day.key" class="chart-day token-day" :data-tokens="day.total" :aria-label="day.key+tr(' Token 明细')" :title="day.label+'：'+(known?day.total.toLocaleString()+' Token':tr('暂无可读取的用量'))" @mouseenter="activeDay=i" @focus="activeDay=i" @blur="activeDay=-1" @click="inspect(day.key)"><span v-show="days<=14||i%Math.ceil(days/7)===0||i===days-1">{{day.label}}</span></button></div><span v-if="!hasData" class="token-curve-empty">{{tokenStore.busy?tr("正在读取本机用量…"):!desktop?tr("请在桌面应用中读取本机 Token 用量"):tokenStore.error?tr("用量读取失败，将在下次检测时重试"):tr("本期未发现 Token 用量记录")}}</span></div></div>
 </section>
 <AppDialog v-if="detail" :title="selectedDay?tr('{date} Token 明细',{date:selectedDay}):tr('近 {days} 天 Token 消耗明细',{days})" :description="tr('统计本机 Codex 与 Claude Code 日志中的用量，按本地日期归档')" wide @close="detail=false"><template #icon><Activity :size="24"/></template>
  <div class="token-dialog-content"><div class="token-detail-summary"><strong>{{known?tokenTotal(detailRows).total.toLocaleString():'—'}} <small>Token</small></strong><span>{{tr("输入")}} {{known?tokenTotal(detailRows).input.toLocaleString():'—'}} {{tr("· 输出")}} {{known?tokenTotal(detailRows).output.toLocaleString():'—'}}</span></div>
  <p class="dialog-info">{{tr("总量 = 输入 + 输出。输入已包含缓存读取和写入，缓存列仅为其中的明细，不重复相加。这里是本机可读取记录的统计，不是账号跨设备总用量或订阅剩余额度。")}}</p>
  <p v-if="tokenStore.error" class="field-message" role="alert">{{tokenStore.error}}</p>
  <div class="token-source-status"><div v-for="source in sources" :key="source.provider"><ProviderIcon :provider="source.provider" size="small"/><span><strong>{{providerName(source.provider)}} · {{tr(sourceText(source.status))}}</strong><small v-for="warning in source.warnings" :key="warning">{{warning}}</small></span></div></div>
  <div class="table-scroll"><table class="token-detail-table"><thead><tr><th>{{tr("日期")}}</th><th>{{tr("提供商")}}</th><th>{{tr("输入（含缓存）")}}</th><th>{{tr("输出")}}</th><th>{{tr("其中缓存读取")}}</th><th>{{tr("其中缓存写入")}}</th><th>{{tr("合计 Token")}}</th></tr></thead><tbody><tr v-for="row in detailRows" :key="row.date+row.provider"><td>{{row.date}}</td><td>{{providerName(row.provider)}}</td><td>{{knownProvider(row.provider)?row.counts.input.toLocaleString():'—'}}</td><td>{{knownProvider(row.provider)?row.counts.output.toLocaleString():'—'}}</td><td>{{knownProvider(row.provider)?row.counts.cache_read.toLocaleString():'—'}}</td><td>{{knownProvider(row.provider)?row.counts.cache_write.toLocaleString():'—'}}</td><td><strong>{{knownProvider(row.provider)?(row.counts.input+row.counts.output).toLocaleString():'—'}}</strong></td></tr></tbody></table><p v-if="!detailRows.length" class="empty-table">{{tr("暂无可读取的本机用量记录")}}</p></div></div>
  <template #footer><span class="dialog-footer-note">{{tokenStore.report?tr("更新于 ")+new Date(tokenStore.report.observed_at).toLocaleTimeString(language):tr("尚未读取用量")}}</span><button :disabled="tokenStore.busy||!desktop" @click="loadTokens(true)">{{tokenStore.busy?tr("正在读取…"):tr("刷新用量")}}</button><button class="primary" @click="detail=false">{{tr("完成")}}</button></template>
 </AppDialog>
</template>
<style scoped>
.token-curve-chart{display:flex;gap:6px;margin:8px 20px 14px 18px;position:relative}
.token-curve-axis{height:150px;width:44px;flex-shrink:0;display:flex;flex-direction:column;justify-content:space-between;padding:3px 0 4px;color:var(--muted);font-size:10px;font-variant-numeric:tabular-nums}
.token-curve-plot{position:relative;flex:1;min-width:0;height:176px}.token-curve-plot svg{position:absolute;inset:0 0 auto;width:100%;height:150px;overflow:visible;pointer-events:none}
.token-grid{stroke:var(--border);stroke-width:1;vector-effect:non-scaling-stroke;stroke-dasharray:3 5}.token-curve-line{fill:none;stroke:var(--primary);stroke-width:2.6;stroke-linecap:round;stroke-linejoin:round;vector-effect:non-scaling-stroke}.token-curve-point{fill:var(--primary);stroke:var(--surface);stroke-width:1.5;vector-effect:non-scaling-stroke}
.token-date-targets{position:absolute;inset:0;display:flex}.token-day{display:flex;flex:1;min-width:0;align-items:flex-end;justify-content:center;padding:0 0 1px;border:0;border-radius:5px;background:transparent;box-shadow:none;font-weight:400;transition:background .12s}.token-day:hover{background:color-mix(in srgb,var(--primary) 4%,transparent);border:0}.token-day:active{transform:none}.token-day>span{font-size:11px;color:var(--muted);white-space:nowrap;line-height:20px}.token-curve-empty{position:absolute;top:60px;left:0;right:0;text-align:center;font-size:12px;color:var(--muted);pointer-events:none}
.home-bottom .token-curve-plot{height:236px}.home-bottom .token-curve-plot svg{height:210px}.home-bottom .token-curve-axis{height:210px}
@media(max-width:640px){.token-controls .select-wrapper{width:120px}.token-curve-chart{margin-left:14px;margin-right:14px}.token-curve-axis{width:38px}.token-day>span{font-size:10px}}
.token-trend{container-type:inline-size}.token-trend .panel-heading{display:grid;grid-template-columns:minmax(0,1fr) auto;gap:10px;align-items:center}.token-controls{display:flex;align-items:center;flex-wrap:nowrap;gap:8px;min-width:0}.token-controls .select-wrapper{width:124px;flex-shrink:0}.token-controls :deep(.select-control){gap:6px;padding-left:8px;padding-right:8px;font-size:12px}.token-controls .select-wrapper+ .select-wrapper{width:90px}.token-trend .panel-heading h2{min-width:0}.token-chart-title{display:flex;justify-content:flex-start;width:100%;min-width:0;padding:0;min-height:28px;border:0;background:none;font-size:17px;font-weight:600;box-shadow:none;white-space:nowrap;gap:4px}.token-chart-title:hover{background:none;color:var(--primary)}.token-chart-title>span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.token-chart-title>svg{flex-shrink:0}.token-trend .panel-heading>.select-wrapper{width:132px}.token-dialog-content{padding:24px 28px}.token-detail-summary{display:flex;align-items:baseline;justify-content:space-between;gap:15px}.token-detail-summary>strong{font-size:26px;font-weight:600}.token-detail-summary small{font-size:13px;color:var(--muted)}.token-detail-summary>span{font-size:13px;color:var(--muted)}.token-source-status{display:flex;gap:18px;margin:20px 0;flex-wrap:wrap}.token-source-status>div{display:flex;gap:9px;align-items:flex-start;flex:1;min-width:220px}.token-source-status strong{font-size:12px;font-weight:500}.token-source-status small{display:block;font-size:11px;color:var(--muted);margin-top:3px}.token-detail-table{font-size:12px;white-space:nowrap}.token-detail-table th,.token-detail-table td{padding:11px 10px}.token-detail-table tbody tr:hover{background:var(--tint)}
@media(max-width:640px){.token-chart-title{font-size:14px}.token-trend .panel-heading>.select-wrapper{width:120px}.token-detail-summary{flex-direction:column;gap:4px}.token-trend .panel-heading{padding-left:14px;padding-right:14px}}
@media(min-width:1024px) and (max-height:810px){
 .home-bottom .token-curve-plot{height:216px}
 .home-bottom .token-curve-plot svg,.home-bottom .token-curve-axis{height:190px}
}
@container(max-width:420px){.token-trend .panel-heading{padding-left:12px;padding-right:12px;gap:6px}.token-chart-title{font-size:14px}.token-controls{gap:6px}.token-controls .select-wrapper{width:112px}.token-controls .select-wrapper+ .select-wrapper{width:82px}.token-controls :deep(.select-control){font-size:11px;gap:4px;padding-left:8px;padding-right:8px}}
</style>
