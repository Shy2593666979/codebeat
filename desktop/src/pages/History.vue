<script setup lang="ts">
import {tr} from '../composables/i18n'
import AppDialog from '../components/AppDialog.vue'
import {ref,computed} from 'vue'
import {History,FileText,RotateCw,X} from 'lucide-vue-next'
import {store,loadLocal} from '../stores/store'
import {api,formatDate} from '../services/api'
import {t} from '../composables/i18n'
import {stateLabel} from '../types/states'
import Select from '../components/Select.vue'
import type {RunRecord} from '../types'
const selected=ref<RunRecord>(),log=ref(''),offset=ref(0),total=ref(0),filter=ref('all'),busy=ref(false)
const filterOptions = computed(()=>[
  { value: 'all', label: tr("全部结果") },
  { value: 'succeeded', label: tr("成功") },
  { value: 'failed', label: tr("失败") },
  { value: 'completed_unverified', label: tr("完成但未验证") },
  { value: 'blocked_approval', label: tr("审批受阻") },
  { value: 'needs_review', label: tr("待人工审查") },
] as const)
const rows=computed(()=>store.runs.filter(r=>filter.value==='all'||r.state===filter.value))
async function open(run:RunRecord,more=false){selected.value=run;busy.value=true;if(!more){log.value='';offset.value=0}try{const chunk=await api.log(run.id,offset.value);log.value+=chunk.text;offset.value=chunk.next_offset;total.value=chunk.total_bytes}catch(e){log.value=String(e)}finally{busy.value=false}}
</script>
<template><div class="page-heading"><div><div class="eyebrow">EVERY RUN HAS A STORY</div><h1>{{t.history}}<span class="heading-dot">.</span></h1><p>{{t.historySub}}</p></div><button @click="loadLocal"><RotateCw :size="15"/>{{tr("刷新")}}</button></div><div class="filter-bar"><Select v-model="filter" :options="filterOptions" :aria-label="tr('运行结果筛选')" :placeholder="tr('选择筛选条件')" /></div><section class="panel"><div v-if="!rows.length" class="empty-state"><span class="empty-icon"><History :size="30"/></span><h3>{{tr("还没有运行记录")}}</h3><p>{{tr("任务执行后，会在这里保留开始、结束、状态及日志。")}}</p></div><div v-else class="table-scroll"><table><thead><tr><th>{{tr("任务")}}</th><th>{{tr("开始 / 结束")}}</th><th>{{tr("结果")}}</th><th>{{tr("诊断")}}</th><th>{{tr("日志")}}</th></tr></thead><tbody><tr v-for="run in rows" :key="run.id"><td><strong>{{store.tasks.find(t=>t.id===run.task_id)?.request.title||run.task_id}}</strong><small>{{tr("第")}} {{run.attempt}} {{tr("次执行")}}</small></td><td>{{formatDate(run.started_at)}}<small>{{formatDate(run.finished_at)}}</small></td><td><span class="badge" :class="run.state">{{stateLabel(run.state)}}</span></td><td>{{run.error_code||'—'}}<small>{{run.error_message}}</small></td><td><button class="icon-button" :title="tr('查看运行日志')" @click="open(run)"><FileText :size="17"/></button></td></tr></tbody></table></div></section><AppDialog v-if="selected" :title="tr('运行日志 · ')+stateLabel(selected.state)" :description="tr('查看本次任务的执行事件与结果')" panel-class="log-dialog" :close-label="tr('关闭日志')" @close="selected=undefined"><template #icon><FileText :size="23"/></template><p>{{tr("默认仅保存事件摘要；完整模型输出需在设置中明确开启。")}}</p><pre>{{log||tr("正在读取日志…")}}</pre><template #footer><button v-if="offset<total" :disabled="busy" @click="open(selected,true)">{{tr("读取更多")}}</button><button class="primary" @click="selected=undefined">{{tr("完成")}}</button></template></AppDialog></template>
