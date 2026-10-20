<script setup lang="ts">
import {tr} from '../composables/i18n'
import {computed} from 'vue'
import {Clock3,Hourglass} from 'lucide-vue-next'
import ProviderIcon from './ProviderIcon.vue'
import ToggleSwitch from './ToggleSwitch.vue'
import ResetCredits from './ResetCredits.vue'
import {store,providerEnabled,toggleProvider} from '../stores/store'
import {formatDate,providerName,usageText,remainingPercent,windowLabel} from '../services/api'
import {relativeTime} from '../services/presentation'
import {t,language} from '../composables/i18n'
import type {ProviderId} from '../types'
const props=withDefaults(defineProps<{provider:ProviderId;compact?:boolean}>(),{compact:false})
const report=computed(()=>store.usage[props.provider]),health=computed(()=>store.providers.find(p=>p.id===props.provider))
const refreshState=computed(()=>store.usageRefresh[props.provider])
const retryLabel=computed(()=>refreshState.value?.retrying?tr(windows.value.length?'上次成功数据 · 自动重试中':'读取失败 · 自动重试中'):tr('监控额度 · 定时继续'))
const windows=computed(()=>report.value?.windows||[])
const next=computed(()=>windows.value.map(w=>w.resets_at_utc).filter((t):t is string=>!!t).sort()[0]||null)
</script>
<template><article class="panel usage-card" :class="[provider,{compact}]"><div class="card-top"><ProviderIcon :provider="provider" size="large"/><div class="provider-title"><h2>{{providerName(provider)}}</h2><p :class="{'usage-retrying':refreshState?.retrying}" :title="refreshState?.retrying?tr('额度更新失败，将持续重试至下一检测周期；成功后自动恢复。'):undefined">{{retryLabel}}</p></div><label class="provider-toggle"><span :title="tr('控制已授权定时任务的后续执行，仍遵循时间和权限设置')">{{tr("自动继续")}}</span><ToggleSwitch :label="providerName(provider)+tr(' 自动继续')" :model-value="providerEnabled(provider)" :disabled="!store.settings" @update:model-value="v=>toggleProvider(provider,v)"/></label></div>
<div class="usage-body"><div class="quota-windows" :class="{multiple:windows.length>1}"><template v-if="windows.length"><div v-for="window in windows" :key="window.label" class="usage-window"><div class="usage-label"><strong>{{t.remaining}} {{remainingPercent(window.used_percent)===null?t.unknown:usageText(remainingPercent(window.used_percent))}}</strong><span>{{windowLabel(window.duration_minutes,window.label,language)}}</span></div><div class="progress" role="progressbar" :aria-label="windowLabel(window.duration_minutes,window.label,language)+' · '+t.remaining" :aria-valuenow="remainingPercent(window.used_percent)??undefined" :aria-valuemin="0" :aria-valuemax="100"><span :style="{width:(remainingPercent(window.used_percent)??0)+'%'}"/></div></div></template><div v-else class="usage-window"><div class="usage-label"><strong>{{tr("剩余额度")}} {{t.unknown}}</strong></div><div class="progress empty"/><p class="usage-explanation">{{(refreshState?.retrying?tr("读取失败，正在自动重试"):tr(report?.message))||(provider==='codex'?tr("连接官方 Codex CLI 或桌面客户端后读取真实额度"):tr("暂无稳定的官方额度读取接口"))}}</p></div></div>
</div>
<div v-if="compact" class="compact-footer" :class="{'with-credits':provider==='codex'}">
 <div class="next-compact"><Clock3 :size="20"/><div class="next-reset-copy"><span>{{tr("下次重置")}}</span><strong>{{relativeTime(next,store.now)}}</strong><small>{{next?formatDate(next):tr("等待官方窗口数据")}}</small></div></div>
 <ResetCredits v-if="provider==='codex'" layout="inline"/>
</div>
<div v-else class="quota-summary" :class="{'with-credits':provider==='codex'}">
 <div class="summary-next"><Clock3/><span>{{tr("下次重置")}}<strong>{{relativeTime(next,store.now)}}</strong><small>{{next?formatDate(next):tr("尚未确认")}}</small></span></div>
 <ResetCredits v-if="provider==='codex'"/>
 <div class="summary-status"><Hourglass/><span>{{tr("数据状态")}}<strong>{{health?.status==='ready'?tr("已连接"):health?.status==='missing'?tr("未安装"):tr("未连接")}}</strong><small>{{refreshState?.retrying?tr(windows.length?"保留上次成功数据":"读取失败，正在自动重试"):report?.status==='available'?tr("官方结构化数据"):tr("额度未知")}}</small></span></div>
</div>
</article></template>

<style scoped>
/* 首页卡片自上而下阅读：先额度，再重置详情和重置卡。 */
.provider-title .usage-retrying{color:var(--orange,#d98a31)}
.usage-card{padding-top:13px;padding-bottom:11px}
.usage-card.compact{padding-top:10px;padding-bottom:7px}
.compact .usage-label>span{font-size:11px}
.compact .usage-body{display:block;margin-top:8px}
.compact .quota-windows.multiple{display:grid;grid-template-columns:minmax(0,1fr);gap:8px;align-items:start}
.compact-footer{display:grid;grid-template-columns:minmax(0,1fr);margin-top:5px;padding-top:5px;border-top:1px solid var(--border);align-items:start}
.compact-footer.with-credits{grid-template-columns:minmax(0,1fr) minmax(0,1.25fr)}
.compact-footer .next-compact{display:flex;flex-direction:row;justify-content:flex-start;align-items:flex-start;gap:8px;width:auto;margin:0;padding:0;border:0;min-width:0}
.next-compact>svg{flex-shrink:0;width:18px;height:18px;margin-top:2px;color:var(--primary)}
.next-reset-copy{min-width:0}
.next-reset-copy>span{display:block;color:var(--muted);font-size:11px;line-height:1.4}
.compact .next-compact strong{display:block;font-size:14px;font-weight:600;line-height:1.5;white-space:normal;margin:1px 0}
.compact .next-compact small{display:block;font-size:11px;line-height:1.5}
.compact-footer>.reset-credits{border-left:1px solid var(--border);padding-left:14px;margin-left:10px;min-height:52px}
.quota-summary{grid-template-columns:repeat(2,minmax(0,1fr));align-items:start}
.quota-summary.with-credits{grid-template-columns:1.15fr 1fr 1fr}
.quota-summary>div:first-child>svg{color:var(--primary);fill:none;stroke:currentColor}
@media(max-width:1100px){
 .compact-footer.with-credits{grid-template-columns:minmax(0,1fr) minmax(0,1.2fr)}
 .compact-footer>.reset-credits{padding-left:9px;margin-left:6px}
 .compact .next-compact strong{font-size:12px}
}
</style>
