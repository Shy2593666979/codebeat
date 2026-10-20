<script setup lang="ts">
import {tr} from '../composables/i18n'
import {onBeforeUnmount,ref,watch,onMounted} from 'vue'
import {Search,RotateCw,MessageSquareMore,LockKeyhole} from 'lucide-vue-next'
import {api} from '../services/api'
import {refreshSessionLocks} from '../services/session-locks'
import type {Conversation,ProviderId} from '../types'
const props=defineProps<{provider:ProviderId;modelValue:Conversation|null}>()
const emit=defineEmits<{'update:modelValue':[value:Conversation|null]}>()
const search=ref(''),items=ref<Conversation[]>([]),busy=ref(false),error=ref(''),warnings=ref<string[]>([])
let lockRefresh:ReturnType<typeof setInterval>
let generation=0,searchTimer:ReturnType<typeof setTimeout>|undefined
async function load(){
 clearTimeout(searchTimer)
 const current=++generation,provider=props.provider,query=search.value
 busy.value=true;error.value='';items.value=[];warnings.value=[]
 const seenCursors=new Set<string>(),seenIds=new Set<string>(),messages=new Set<string>()
 let cursor:string|null=null
 try{
  do{
   const result=await api.sessions(provider,query,cursor)
   if(current!==generation)return
   items.value.push(...result.data.filter(item=>{if(seenIds.has(item.session_id))return false;seenIds.add(item.session_id);return true}))
   result.warnings.forEach(message=>messages.add(message));warnings.value=[...messages]
   cursor=result.next_cursor
   if(cursor){if(seenCursors.has(cursor))throw new Error('会话分页游标重复，请刷新重试。');seenCursors.add(cursor)}
  }while(cursor)
  if(props.modelValue){const selected=items.value.find(c=>c.session_id===props.modelValue?.session_id);if(selected)emit('update:modelValue',selected)}
 }catch(e){if(current===generation)error.value=String(e)}finally{if(current===generation)busy.value=false}
}
watch(search,()=>{generation++;clearTimeout(searchTimer);searchTimer=setTimeout(()=>void load(),250)},{flush:'sync'})
watch(()=>props.provider,()=>{if(props.modelValue?.provider!==props.provider)emit('update:modelValue',null);items.value=[];search.value='';clearTimeout(searchTimer);void load()},{immediate:true})
onMounted(()=>{lockRefresh=setInterval(()=>{if(!document.hidden)void refreshSessionLocks(items.value)},5000)})
onBeforeUnmount(()=>{generation++;clearTimeout(searchTimer);clearInterval(lockRefresh)})
</script>
<template><div class="session-picker"><label>{{tr("历史会话")}} <span class="required">*</span></label><div class="session-search"><Search :size="16"/><input v-model="search" :placeholder="tr('搜索已有对话')" :aria-label="tr('搜索已有对话')" @keydown.enter.prevent="load()"><button type="button" class="icon-button" :title="tr('搜索会话')" :aria-label="tr('刷新可恢复会话')" :disabled="busy" @click="load()"><RotateCw :size="15" :class="{spin:busy}"/></button></div><div v-if="error" class="field-message" role="alert">{{tr(error)}}</div><div v-if="busy&&!items.length" class="session-empty">{{tr("正在读取本地会话…")}}</div><div v-else-if="!items.length" class="session-empty"><MessageSquareMore :size="24"/><span>{{tr("没有可选会话")}}</span><small>{{tr("请先在官方工具中创建对话，再刷新此列表。")}}</small></div><div v-else class="session-list" role="listbox" :aria-label="tr('可恢复会话')"><button v-for="c in items" :key="c.session_id" type="button" role="option" :aria-selected="modelValue?.session_id===c.session_id" :disabled="!c.resumable" :class="{selected:modelValue?.session_id===c.session_id}" @click="emit('update:modelValue',c)"><MessageSquareMore class="session-option-icon" :size="20"/><span class="session-option-copy"><span class="session-option-title"><strong>{{c.title}}</strong></span><small class="session-option-path" :title="c.cwd">{{c.cwd}}</small><small v-if="c.lock_state==='unknown'">{{tr('占用状态未知')}}</small><small v-if="!c.resumable">{{tr("不可恢复")}}</small></span><LockKeyhole v-if="c.lock_state==='in_use'" class="session-option-lock" :size="17" :aria-label="tr('使用中')"><title>{{tr('使用中')}}</title></LockKeyhole></button></div><small v-for="w in warnings" :key="w" class="field-message">{{tr(w)}}</small></div></template>
