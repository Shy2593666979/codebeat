<script setup lang="ts">
import {computed} from 'vue'
import {ChevronLeft,ChevronRight} from 'lucide-vue-next'
import Select from './Select.vue'
import {tr} from '../composables/i18n'
const props=withDefaults(defineProps<{total:number;selected:number;page:number;pageSize:number;subject:'tasks'|'sessions';busy?:boolean;partial?:boolean}>(),{busy:false,partial:false})
const emit=defineEmits<{'update:page':[value:number];'update:pageSize':[value:number]}>()
const totalPages=computed(()=>Math.max(1,Math.ceil(props.total/props.pageSize)))
const pageNumbers=computed(()=>Array.from({length:Math.min(5,totalPages.value)},(_,i)=>Math.max(1,Math.min(props.page-2,totalPages.value-4))+i))
const options=computed(()=>[10,20,50].map(value=>({value,label:tr('每页 {count} 条',{count:value})})))
const summary=computed(()=>tr(props.subject==='tasks'?'共 {count} 个任务，已选择 {selected} 个':props.partial?'已加载 {count} 个会话，已选择 {selected} 个':'共 {count} 个会话，已选择 {selected} 个',{count:props.total,selected:props.selected}))
function go(page:number){if(!props.busy)emit('update:page',Math.max(1,Math.min(totalPages.value,page)))}
</script>
<template><footer class="record-pagination"><span role="status">{{summary}}<small v-if="busy"> · {{tr('加载中…')}}</small></span><slot name="summary-actions"/><nav :aria-label="subject==='tasks'?tr('任务分页'):tr('会话分页')"><button class="page-step" :disabled="busy||page===1" :aria-label="tr('上一页')" @click="go(page-1)"><ChevronLeft/></button><button v-for="number in pageNumbers" :key="number" :class="{active:page===number}" :disabled="busy" :aria-label="tr('第 {page} 页',{page:number})" :aria-current="page===number?'page':undefined" @click="go(number)">{{number}}</button><button class="page-step" :disabled="busy||page===totalPages" :aria-label="tr('下一页')" @click="go(page+1)"><ChevronRight/></button></nav><Select :model-value="pageSize" :options="options" :searchable="false" :disabled="busy" :aria-label="subject==='tasks'?tr('每页任务数'):tr('每页会话数')" @update:model-value="emit('update:pageSize',$event)"/></footer></template>
<style scoped>
.record-pagination{display:flex;flex-shrink:0;align-items:center;gap:14px;padding:14px 13px;border-top:1px solid var(--border);font-size:12px;color:var(--muted);min-height:60px}.record-pagination>span{margin-right:auto}.record-pagination>span>small{font-size:11px}.record-pagination nav{display:flex;gap:5px}.record-pagination nav button{padding:3px 6px;min-width:29px;height:30px;min-height:30px;border-radius:7px;font-size:12px;background:var(--surface)}.record-pagination nav button.active{background:var(--primary);color:#fff;border-color:var(--primary)}.record-pagination nav svg{width:15px;height:15px}.record-pagination :deep(.select-wrapper){width:108px;flex-shrink:0}.record-pagination :deep(.select-control){min-height:32px;height:32px;font-size:11px;padding:5px 8px;gap:4px}.record-pagination :deep(.select-chevron){width:14px;height:14px}
@media(max-width:640px){.record-pagination{flex-wrap:wrap;gap:9px}.record-pagination>span{flex-basis:100%}.record-pagination nav{margin-right:auto}}
</style>
