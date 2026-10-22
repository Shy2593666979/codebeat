<script setup lang="ts">
import {computed,onMounted,onUnmounted,ref,watch} from 'vue'
import {Check,Play,Clock3} from 'lucide-vue-next'
import {tr} from '../composables/i18n'
import Select from './Select.vue'
import ToggleSwitch from './ToggleSwitch.vue'
import ProviderIcon from './ProviderIcon.vue'
import CodingAllocationIcon from './CodingAllocationIcon.vue'
import {call,desktop,formatDate,providerName} from '../services/api'
import {minuteClock,planPretrigger} from '../services/pretrigger'
import {allocationDate,allocationRows,localDay,settingsForDay,shiftMinute,rulerLabels} from '../services/coding-allocation'
import type {AllocationNode} from '../services/coding-allocation'
import type {PretriggerSettings,OptimizerRuntime,ConfirmedWindow} from '../types'

const enabled=defineModel<boolean>('enabled',{required:true})
const model=defineModel<PretriggerSettings>({required:true})
const props=defineProps<{paused?:boolean}>()
const now=ref(Date.now()),runtimes=ref<OptimizerRuntime[]>([]),windows=ref<ConfirmedWindow[]>([]),readError=ref('')
const options=computed(()=>[{value:'weekdays',label:tr('工作日（周一至周五）')},{value:'daily',label:tr('每天（含周末）')}])
const error=ref('')
const date=computed(()=>{try{return allocationDate(model.value,now.value)}catch{return localDay(now.value,model.value.timezone)}})
const effective=computed(()=>settingsForDay(model.value,date.value))
const plan=computed(()=>{try{return planPretrigger(effective.value)}catch{return null}})
watch(effective,value=>{try{planPretrigger(value);error.value=''}catch(e){error.value=String(e).replace('Error: ','')}},{deep:true,immediate:true})
const rows=computed(()=>plan.value?allocationRows(model.value,date.value,now.value,runtimes.value,windows.value):[])
const nodes=computed(()=>rows.value.flatMap(row=>row.nodes).sort((a,b)=>a.minute-b.minute))
const heading=computed(()=>date.value===localDay(now.value,model.value.timezone)?tr('今日计划'):tr('下次工作计划'))
function missed(node:AllocationNode){return node.executeMinute!==null&&shiftMinute(new Date(now.value).toISOString(),date.value,model.value.timezone)>node.executeMinute+10}
function nodeState(node:AllocationNode){
 if(!node.confirmed)return missed(node)?'missed':'upcoming'
 const cycle=rows.value.find(row=>row.provider===node.provider)?.cycles.find(cycle=>cycle.minute===node.minute)
 const minute=shiftMinute(new Date(now.value).toISOString(),date.value,model.value.timezone)
 return cycle&&minute<cycle.b?'active':'completed'
}
function stateLabel(node:AllocationNode){return tr(({active:'进行中',completed:'已完成',upcoming:'待开始',missed:'未触发'} as const)[nodeState(node)])}
function nodeTitle(node:AllocationNode){return providerName(node.provider)+' · '+(node.events.length?node.events.map(e=>tr(e.source?.startsWith('app')?'自动触发':'检测到使用')+' · '+formatDate(e.start)).join('\n'):node.executeMinute===null?tr('本轮无需额外触发'):tr('计划执行：{time}',{time:minuteClock(node.executeMinute)}))}
function cycleTitle(node:AllocationNode){return node.events.map(e=>tr(e.source?.startsWith('app')?'自动触发':'检测到使用')+' · '+formatDate(e.start)+' – '+formatDate(e.closed_at||e.end)).join('\n')}
const activity=computed(()=>{
 if(!enabled.value)return tr('额度调配已关闭')
 if(props.paused)return tr('自动执行已暂停')
 if(!model.value.auto_trigger)return tr('自动触发已暂停')
 if(runtimes.value.some(r=>r.date===date.value&&r.status==='verifying'))return tr('正在核实窗口变化，暂不自动触发')
 const active=rows.value.flatMap(row=>row.actual).filter(w=>Date.parse(w.closed_at||w.end)>now.value).sort((a,b)=>Date.parse(b.start)-Date.parse(a.start))[0]
 if(active?.source==='app_unconfirmed')return tr('已触发，预计 {time} 恢复',{time:minuteClock(active.b)})
 if(active)return tr('窗口计时中 · {time} 恢复',{time:minuteClock(active.b)})
 if(readError.value||runtimes.value.some(r=>r.date===date.value&&r.status==='unknown'))return tr('等待额度确认，暂不自动触发')
 return tr('按计划自动检查额度')
})
function toggleEnabled(value:boolean){if(value)model.value={...model.value,mode:'balanced',auto_trigger:true,wake:true};enabled.value=value}
function toggleProvider(provider:'codex'|'claude_code'){model.value={...model.value,providers:model.value.providers.includes(provider)?model.value.providers.filter(p=>p!==provider):[...model.value.providers,provider]}}
const timeline=computed(()=>{
 if(!plan.value)return null
 const low=Math.min(plan.value.intervals[0]![0],...nodes.value.map(n=>n.minute))
 const ends=rows.value.flatMap(row=>[...row.cycles.map(w=>w.b),...row.predictions.map(w=>w.b)])
 const cycleEnd=ends.length?Math.max(...ends):Math.max(...nodes.value.map(n=>n.minute+300))
 const high=Math.max(cycleEnd,plan.value.intervals.at(-1)![1])
 if(!Number.isFinite(low)||!Number.isFinite(high))return null
 return {low,high,cycleEnd,width:Math.max(1,high-low)}
})
const rulerTrack=ref<HTMLElement>(),rulerWidth=ref(250)
let rulerObserver:ResizeObserver|undefined
watch(rulerTrack,el=>{rulerObserver?.disconnect();if(el){rulerObserver=new ResizeObserver(entries=>{rulerWidth.value=entries[0]!.contentRect.width});rulerObserver.observe(el)}})
const workAxis=computed(()=>timeline.value&&plan.value?rulerLabels(plan.value.intervals.flat(),timeline.value.low,timeline.value.high,rulerWidth.value):{labels:[],height:20})
const codexAxis=computed(()=>timeline.value?rulerLabels([...nodes.value.map(n=>n.minute),...rows.value.flatMap(row=>row.resets),timeline.value.cycleEnd],timeline.value.low,timeline.value.high,rulerWidth.value):{labels:[],height:20})
function ticks(values:number[]){return timeline.value?[...new Set(values)].sort((a,b)=>a-b).slice(1,-1).map(minute=>({minute,left:((minute-timeline.value!.low)/timeline.value!.width*100)+'%'})):[]}
function rowTicks(row:typeof rows.value[number]){return ticks([...row.nodes.map(n=>n.minute),...row.cycles.map(w=>w.b),...row.predictions.map(w=>w.b)])}
function segment(a:number,b:number){const t=timeline.value!;const left=Math.max(a,t.low),right=Math.min(b,t.high);return {left:((left-t.low)/t.width*100)+'%',width:(Math.max(0,right-left)/t.width*100)+'%'}}
let clock:ReturnType<typeof setInterval>|undefined,poll:ReturnType<typeof setInterval>|undefined,revision=0
async function load(){if(!desktop)return;const selected=date.value,id=++revision;try{const status=await call<{plans:OptimizerRuntime[];windows?:ConfirmedWindow[]}>('get_optimizer_status');if(id!==revision||selected!==date.value)return;runtimes.value=status.plans;windows.value=status.windows??[];readError.value=''}catch{if(id===revision)readError.value=tr('暂时无法更新额度状态')}}
watch(date,()=>{runtimes.value=[];windows.value=[];void load()})
watch(()=>JSON.stringify(model.value),()=>void load())
onMounted(()=>{void load();clock=setInterval(()=>now.value=Date.now(),1000);poll=setInterval(()=>void load(),15000)})
onUnmounted(()=>{revision++;clearInterval(clock);clearInterval(poll);rulerObserver?.disconnect()})
</script>

<template>
 <section class="panel settings-card pretrigger-settings">
  <header class="probe-heading"><h2><CodingAllocationIcon/>{{tr('每日Coding 额度调配')}}</h2><ToggleSwitch :label="tr('每日Coding 额度调配')" :model-value="enabled" @update:model-value="toggleEnabled"/></header>
  <div class="pretrigger-fields">
   <div class="probe-field probe-days"><label for="probe-days">{{tr('生效日期')}}</label><Select id="probe-days" v-model="model.days" :options="options" :aria-label="tr('预启动生效日期')"/></div>
   <div class="probe-field"><span>{{tr('工作时间')}}</span><div class="probe-time-range"><input type="time" required v-model="model.work_start" :aria-label="tr('工作开始时间')"><span>{{tr('至')}}</span><input type="time" required v-model="model.work_end" :aria-label="tr('工作结束时间')"></div></div>
   <div class="probe-field"><label class="probe-lunch"><span>{{tr('启用午休时间')}}</span><ToggleSwitch :label="tr('预启动午休')" v-model="model.lunch_enabled"/></label><div v-if="model.lunch_enabled" class="probe-time-range"><input type="time" required v-model="model.lunch_start" :aria-label="tr('午休开始时间')"><span>{{tr('至')}}</span><input type="time" required v-model="model.lunch_end" :aria-label="tr('午休结束时间')"></div></div>
   <div class="probe-providers"><button v-for="provider in (['codex','claude_code'] as const)" :key="provider" type="button" :aria-pressed="model.providers.includes(provider)" @click="toggleProvider(provider)"><ProviderIcon :provider="provider" size="small"/>{{providerName(provider)}}</button></div>
   <p v-if="error" role="alert" class="field-message">{{tr(error)}}</p>
   <p v-if="readError" class="probe-note">{{readError}}</p>
   <div v-if="plan" class="probe-plan"><strong>{{heading}} <i>·</i> {{date}}</strong><p class="probe-activity" role="status">{{activity}}</p><div class="probe-times"><span v-for="node in nodes" :key="node.provider+':'+node.minute" :class="[nodeState(node),{confirmed:node.confirmed}]" :title="nodeTitle(node)"><span class="node-status"><span class="node-icon"><Check v-if="nodeState(node)==='completed'"/><Play v-else-if="nodeState(node)==='active'"/><Clock3 v-else/></span><em>{{stateLabel(node)}}</em></span><b>{{minuteClock(node.minute)}}</b><small>{{node.confirmed?tr('5H额度触发'):node.executeMinute===null?tr('本轮无需额外触发'):missed(node)?tr('未触发'):tr('预计触发')}}</small></span></div><p v-if="!nodes.length" class="probe-note">{{tr('暂无推荐时间')}}</p></div>
   <div v-if="plan&&timeline" class="optimizer-timeline" role="img" :aria-label="tr('工作与额度窗口时间线')">
    <div class="timeline-axis work-axis" :style="{height:workAxis.height+'px'}"><span v-for="tick in workAxis.labels" :key="tick.minute" :style="tick.style">{{minuteClock(tick.minute)}}</span></div>
    <div class="timeline-row" :aria-label="tr('工作时段')"><div ref="rulerTrack" class="timeline-track work-track"><i v-for="tick in ticks(plan.intervals.flat())" :key="tick.minute" class="timeline-gridline" :style="{left:tick.left}"/><span v-if="plan.intervals.length===2" class="lunch-segment" :style="segment(plan.intervals[0]![1],plan.intervals[1]![0])"/><span v-for="[a,b] in plan.intervals" :key="a" class="work-segment" :style="segment(a,b)"/></div></div>
    <div v-for="row in rows" :key="row.provider" class="timeline-row" :aria-label="providerName(row.provider)"><div class="timeline-track"><i v-for="tick in rowTicks(row)" :key="tick.minute" class="timeline-gridline" :class="{reset:row.resets.includes(tick.minute)}" :style="{left:tick.left}"/><span v-for="window in row.predictions" :key="window.display" class="theory-segment" :title="providerName(row.provider)+' · '+minuteClock(window.a)+' – '+minuteClock(window.b)" :style="segment(window.a,window.b)"/><span v-for="window in row.cycles" :key="window.minute" class="actual-segment" :class="nodeState(window)" :title="providerName(row.provider)+' · '+cycleTitle(window)" :style="segment(window.a,window.b)"/></div></div>
    <div class="timeline-axis codex-axis" :style="{height:codexAxis.height+'px'}"><span v-for="tick in codexAxis.labels" :key="tick.minute" :class="{reset:rows.some(row=>row.resets.includes(tick.minute))}" :title="rows.some(row=>row.resets.includes(tick.minute))?tr('官方重置'):undefined" :style="tick.style">{{minuteClock(tick.minute)}}</span></div>
    <div class="timeline-legend"><span class="work-key">{{tr('工作时段')}}</span><span v-if="effective.lunch_enabled" class="lunch-key">{{tr('午休')}}</span><span class="completed-key">{{tr('已完成')}}</span><span class="actual-key">{{tr('进行中')}}</span><span class="theory-key">{{tr('预计触发')}}</span></div>
   </div>
  </div>
 </section>
</template>
<style scoped>
.pretrigger-settings{--allocation-active:#087cff;--allocation-completed:#8bbcff;--allocation-soft:color-mix(in srgb,#789fca 12%,var(--surface));padding:17px 18px;align-self:start;min-width:0}
.probe-heading{display:flex;align-items:center;justify-content:space-between;gap:6px;border-bottom:1px solid var(--border);padding-bottom:11px}
.probe-heading h2{display:flex;align-items:center;gap:7px;font-size:15px;margin:0;white-space:nowrap}
.probe-heading h2 svg{width:24px;height:24px;color:var(--primary);flex-shrink:0}
.probe-heading :deep(.toggle-switch){transform:scale(.85);transform-origin:right center}
.pretrigger-fields{display:grid;gap:10px;padding-top:13px}
.probe-field{display:grid;gap:8px;font-size:13px}
.probe-days{grid-template-columns:auto minmax(0,1fr);align-items:center;gap:12px}
.probe-field :deep(.select-control){min-height:32px;height:32px;font-size:11px;padding:4px 8px}
.probe-time-range{display:flex;align-items:center;gap:7px;min-width:0}
.probe-time-range input{width:0;flex:1;min-width:0;padding:5px 7px;font-size:13px;height:36px}
.probe-time-range>span{color:var(--muted);font-size:11px}
.probe-lunch{display:flex;align-items:center;justify-content:space-between;min-height:24px}
.probe-lunch :deep(.toggle-switch){transform:scale(.85);transform-origin:right center}
.probe-providers{display:flex;gap:7px;flex-wrap:wrap}
.probe-providers button{display:flex;align-items:center;justify-content:center;gap:6px;padding:6px;font-size:11px;flex:1;min-height:33px;white-space:nowrap}
.probe-providers button[aria-pressed=true]{border-color:var(--primary);background:var(--tint);color:var(--primary)}
.probe-providers :deep(.provider-mark){width:19px;height:19px}
.probe-plan{line-height:1.4;background:color-mix(in srgb,var(--allocation-active) 4%,var(--surface));border:1px solid var(--border);border-radius:14px;padding:12px}
.probe-plan>strong{font-size:13px}.probe-plan>strong i{color:var(--allocation-active);font-style:normal;margin:0 4px}
.probe-activity{font-size:12px;line-height:1.5;color:var(--muted);margin:6px 0 0;overflow-wrap:anywhere}
.probe-times{display:grid;grid-template-columns:repeat(auto-fit,minmax(84px,1fr));gap:8px;margin-top:12px}
.probe-times>span{display:flex;flex-direction:column;gap:7px;min-width:0;padding:11px 8px;border:1px solid transparent;border-radius:11px;background:var(--allocation-soft)}
.probe-times b{font-size:clamp(23px,2.4vw,30px);line-height:1.2;color:var(--text);font-variant-numeric:tabular-nums;letter-spacing:-.8px;white-space:nowrap}
.probe-times small{font-size:11px;color:var(--muted)}
.node-status{display:flex;align-items:center;gap:6px;min-height:25px;color:var(--muted)}
.node-status em{font-style:normal;font-size:10px;line-height:1.2;overflow-wrap:anywhere}
.node-icon{display:grid;place-items:center;width:25px;height:25px;flex-shrink:0;border-radius:50%;background:color-mix(in srgb,var(--allocation-completed) 20%,transparent)}
.node-icon svg{width:14px;height:14px;stroke-width:2.6}
.probe-times>.completed{background:color-mix(in srgb,var(--allocation-completed) 17%,var(--surface))}
.probe-times>.active{background:linear-gradient(145deg,#318fff,var(--allocation-active));box-shadow:0 4px 12px #087cff1a}
.probe-times>.active b,.probe-times>.active small,.probe-times>.active .node-status{color:white}
.probe-times>.active .node-status em{background:#ffffff26;padding:3px 7px;border-radius:20px}
.probe-times>.active .node-icon{background:#ffffff29}.probe-times>.active .node-icon svg{fill:currentColor;stroke-width:1.5}
.probe-times>.upcoming{border:1px dashed color-mix(in srgb,var(--allocation-active) 32%,var(--border))}
.probe-times>.missed b{color:var(--muted)}
.probe-note{font-size:11px;line-height:1.6;color:var(--muted);margin:0}
.optimizer-timeline{line-height:1.4;padding:14px 10px;border:1px solid var(--border);border-radius:14px}
.timeline-axis{position:relative;height:18px;margin:0 0 7px;font-size:11px;color:var(--muted)}
.timeline-axis>span{position:absolute;white-space:nowrap;line-height:18px;font-variant-numeric:tabular-nums}
.timeline-axis{border-bottom:1px solid var(--border)}
.codex-axis{border-bottom:0;border-top:1px solid var(--border);padding-top:2px}
.timeline-row{margin:9px 0;min-width:0}
.timeline-track{position:relative;height:30px;background:var(--allocation-soft);border-radius:7px;overflow:hidden}
.timeline-gridline{position:absolute;top:0;width:1px;height:100%;transform:translateX(-50%);background:color-mix(in srgb,var(--surface) 75%,transparent);z-index:2;pointer-events:none}
.timeline-gridline.reset{background:var(--orange)}
.codex-axis>span.reset{color:var(--orange);font-weight:600}
.timeline-track>span{position:absolute;top:0;height:100%;font-size:11px;display:grid;place-items:center;overflow:hidden;white-space:nowrap}
.timeline-track>.theory-segment,.timeline-track>.actual-segment{display:flex;align-items:center;justify-content:flex-start;padding-left:4px}
.work-segment{background:#4db69b}.lunch-segment{background:#f2bc66}
.theory-segment{background:var(--allocation-soft);color:var(--muted);border:1px dashed color-mix(in srgb,var(--allocation-active) 48%,transparent)}
.actual-segment{background:var(--allocation-completed);color:white;z-index:1}
.actual-segment.active{background:var(--allocation-active)}
.timeline-legend{display:flex;gap:6px 10px;flex-wrap:wrap;font-size:10px;color:var(--muted);margin-top:10px}
.timeline-legend span:before{content:'';display:inline-block;width:10px;height:10px;margin-right:4px;border-radius:3px;vertical-align:-1px}
.work-key:before{background:#4db69b}.lunch-key:before{background:#f2bc66}.completed-key:before{background:var(--allocation-completed)}.theory-key:before{background:var(--allocation-soft);border:1px dashed var(--allocation-completed)}.actual-key:before{background:var(--allocation-active)}
@media(max-width:1100px){.pretrigger-settings{padding:13px 10px}.probe-heading h2{font-size:13px;gap:5px}.probe-heading h2 svg{width:21px;height:21px}.pretrigger-fields{gap:8px;padding-top:10px}.probe-field{gap:7px;font-size:12px}.probe-time-range input{height:33px;font-size:12px}.probe-days{gap:7px}}
@media(min-width:851px) and (max-height:900px){.pretrigger-settings{padding:13px 12px}.probe-heading{padding-bottom:9px}.pretrigger-fields{gap:7px;padding-top:10px}.probe-field{gap:6px}.probe-plan{padding:10px}.probe-times{margin-top:9px}.probe-times>span{padding:9px 7px;gap:6px}.optimizer-timeline{padding:10px}.timeline-row{margin:7px 0}.timeline-axis{margin-bottom:5px}.timeline-legend{margin-top:8px}}
@media(min-width:851px) and (max-width:1100px) and (max-height:760px){.pretrigger-settings{padding:10px}.probe-heading{padding-bottom:7px}.pretrigger-fields{gap:6px;padding-top:8px}.probe-plan{padding:8px}.probe-activity{margin-top:4px;font-size:11px}.probe-times{margin-top:5px;gap:5px}.probe-times>span{padding:5px 3px;gap:2px}.optimizer-timeline{padding:8px}.timeline-row{margin:5px 0}.timeline-axis{margin-bottom:3px}.timeline-legend{margin-top:6px;gap:4px 7px;font-size:9px}}
</style>
