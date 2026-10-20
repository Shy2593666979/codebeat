<script setup lang="ts">
import {tr} from '../composables/i18n'
import {computed,ref,watch,onBeforeUnmount} from 'vue'
import {CalendarClock,MessageSquareMore,Clock3,Check} from 'lucide-vue-next'
import SessionPicker from './SessionPicker.vue'
import AppDialog from './AppDialog.vue'
import ProviderIcon from './ProviderIcon.vue'
import Select from './Select.vue'
import {api,canSchedule,localInput} from '../services/api'
import {store,loadLocal} from '../stores/store'
import {refreshSessionLocks} from '../services/session-locks'
import type {Conversation,ProviderId,Task} from '../types'
const props=defineProps<{task?:Task;conversation?:Conversation;immediate?:boolean;modal?:boolean}>(),emit=defineEmits<{close:[];saved:[]}>()
const provider=ref<ProviderId>('codex'),conversation=ref<Conversation|null>(null),title=ref(''),prompt=ref(''),time=ref(localInput()),lockBehavior=ref<'fork'|'pause'>('fork'),busy=ref(false),error=ref('')
const timezone=Intl.DateTimeFormat().resolvedOptions().timeZone
const repeat=ref('none'),repeatHours=ref(6)
const repeatOptions = computed(()=>[
  { value: 'none', label: tr("仅执行一次") },
  { value: 'daily', label: tr("每天") },
  { value: 'weekly', label: tr("每周") },
  { value: 'every_hours', label: tr("按小时间隔") },
] as const)
if(store.settings){const next=new Date();const [h,m]=store.settings.default_time.split(':').map(Number);next.setHours(h,m,0,0);if(next<=new Date())next.setDate(next.getDate()+1);time.value=localInput(next)}
watch(()=>props.conversation,c=>{if(c){provider.value=c.provider;conversation.value=c;prompt.value='';title.value=tr('继续「{title}」',{title:c.title})}},{immediate:true})
watch(()=>props.task,task=>{repeat.value=task?.request.recurrence?.kind||'none';if(task?.request.recurrence?.kind==='every_hours')repeatHours.value=task.request.recurrence.hours},{immediate:true})
watch(()=>props.task,task=>{if(task){const r=task.request;provider.value=r.conversation.provider;conversation.value=r.conversation;title.value=r.title;prompt.value=r.prompt;time.value=localInput(new Date(r.scheduled_at_utc));lockBehavior.value=r.locked_session_behavior||'fork';}},{immediate:true})
const valid=computed(()=>canSchedule(conversation.value)&&title.value.trim()&&prompt.value.trim()&&prompt.value.length<=4000&&(props.immediate||time.value)&& (repeat.value!=='every_hours'||Number.isInteger(repeatHours.value)&&repeatHours.value>=1&&repeatHours.value<=168))
const dialogTitle=computed(()=>props.task?'编辑任务':props.immediate?'立即继续会话':'新建任务')
watch(()=>conversation.value?.provider+':'+conversation.value?.session_id,()=>{if(conversation.value)void refreshSessionLocks([conversation.value])},{immediate:true})
const lockTimer=setInterval(()=>{if(!document.hidden&&conversation.value)void refreshSessionLocks([conversation.value])},5000)
onBeforeUnmount(()=>clearInterval(lockTimer))
async function save(){if(!valid.value||!conversation.value||busy.value)return;busy.value=true;error.value='';try{await refreshSessionLocks([conversation.value]);if(conversation.value.provider==='codex'&&conversation.value.lock_state==='unknown')throw new Error(tr('无法确认会话占用状态，请刷新后重试'));const saved=await api.saveTask({locked_session_behavior:lockBehavior.value,recurrence:repeat.value==='none'?null:repeat.value==='every_hours'?{kind:'every_hours',hours:repeatHours.value}:{kind:repeat.value as 'daily'|'weekly'},conversation:conversation.value,title:title.value.trim(),prompt:prompt.value,scheduled_at_utc:new Date(props.immediate?Date.now()+60000:time.value).toISOString(),scheduled_local:props.immediate?localInput(new Date(Date.now()+60000)):time.value,timezone_id:timezone,wake_to_run:true,catch_up_policy:'ask',catch_up_ttl_minutes:720,permission_profile:'full_access',consent:true,notify:true},props.task?.id);if(props.immediate)await api.run(saved.id);await loadLocal();store.noticeTopic=`task:${saved.id}:registration`;store.notice='任务已注册到 Windows 任务计划程序';emit('saved');}catch(e){error.value=String(e);await loadLocal()}finally{busy.value=false}}
</script>
<template>
 <AppDialog :title="dialogTitle" :description="immediate?tr('选择会话，确认要继续完成的工作'):tr('选择已有会话，安排下一次继续工作的时间')" wide panel-class="task-editor-dialog" :busy="busy" @close="emit('close')">
  <template #icon><CalendarClock :size="28"/></template>
  <form id="task-editor-form" class="task-editor-form" :aria-busy="busy" :inert="busy||undefined" @submit.prevent="save">
   <aside class="editor-session-column">
    <div class="form-section-heading"><MessageSquareMore :size="22"/><h3>{{tr("选择会话")}}</h3><span>01</span></div>
    <label class="form-label">{{tr("AI 提供商")}}</label>
    <div class="provider-choice editor-provider-choice">
     <button v-for="id in (['codex','claude_code'] as const)" :key="id" type="button" :class="{selected:provider===id}" :aria-pressed="provider===id" :aria-label="id==='codex'?'Codex':'Claude Code'" @click="provider=id"><ProviderIcon :provider="id"/><span class="provider-choice-copy"><strong>{{id==='codex'?'Codex':'Claude Code'}}</strong><small>{{id==='codex'?tr("强大的代码助手"):tr("专注编程与分析")}}</small></span><span class="provider-choice-radio"><Check v-if="provider===id" :size="12"/></span></button>
    </div>
    <SessionPicker v-model="conversation" :provider="provider"/>
   </aside>
   <div class="editor-fields-column">
    <div class="editor-fields-scroll">
     <div class="form-section-heading"><CalendarClock :size="23"/><h3>{{tr("任务安排")}}</h3><span>02</span></div>
     <label for="task-title" class="form-label">{{tr("任务名称")}} <span class="required">*</span></label><input id="task-title" v-model="title" maxlength="120" required :placeholder="tr('输入任务名称')">
     <label for="task-prompt" class="form-label">{{tr("提示词")}} <span class="required">*</span></label><div class="textarea-wrap"><textarea id="task-prompt" v-model="prompt" rows="4" maxlength="4000" required :placeholder="tr('描述希望 AI 接下来完成的工作…')"/><span>{{prompt.length}} / 4000</span></div>
     <div v-if="!immediate" class="editor-time-grid"><div><label for="task-time" class="form-label">{{tr("执行时间")}} <span class="required">*</span></label><input id="task-time" v-model="time" type="datetime-local" :min="localInput(new Date())" required></div><div><label for="task-repeat" class="form-label">{{tr("重复执行")}}</label><Select v-model="repeat" :options="repeatOptions" :placeholder="tr('选择重复方式')" id="task-repeat" :aria-label="tr('重复执行')" /></div></div>
     <p v-if="immediate" class="input-hint"><Clock3 :size="14"/>{{tr("确认后立即启动")}}</p>
     <label v-if="repeat==='every_hours'&&!immediate" class="interval-field">{{tr("每")}} <input v-model.number="repeatHours" type="number" min="1" max="168" required :aria-label="tr('重复间隔小时数')"> {{tr("小时执行一次")}}</label><p v-if="repeat!=='none'&&!immediate" class="input-hint">{{tr("周期任务会重复消耗额度；失败或待确认时暂停。")}}</p>
     <fieldset class="session-lock-policy"><legend class="form-label">{{tr('会话锁定策略')}}</legend><div class="lock-behavior-options"><label :class="{selected:lockBehavior==='fork'}" :title="tr('执行时若仍被占用，将复制已有对话历史，在新分叉中继续。')"><input v-model="lockBehavior" type="radio" name="session-lock-behavior" value="fork"><span>{{tr('创建聊天分叉')}}</span></label><label :class="{selected:lockBehavior==='pause'}" :title="tr('执行时若仍被占用，暂停任务，等待你手动恢复。')"><input v-model="lockBehavior" type="radio" name="session-lock-behavior" value="pause"><span>{{tr('暂停任务')}}</span></label></div></fieldset>
     <p v-if="conversation?.lock_state==='unknown'" class="field-message">{{tr('无法确认会话占用状态，请刷新后重试')}}</p>
     <p v-if="error" class="field-message" role="alert">{{error}}</p>
    </div>
    <footer class="editor-actions"><button type="button" :disabled="busy" @click="emit('close')">{{tr("取消")}}</button><button type="submit" class="primary" :disabled="!valid||busy">{{busy?tr("正在保存…"):immediate?tr("立即继续"):task?tr("确认修改"):tr("确认创建")}}</button></footer>
   </div>
  </form>
 </AppDialog>
</template>
