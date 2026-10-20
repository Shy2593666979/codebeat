<script setup lang="ts">
import {tr} from '../composables/i18n'
import AppDialog from '../components/AppDialog.vue'
import PaginationBar from '../components/PaginationBar.vue'
import SessionContextMenu from '../components/SessionContextMenu.vue'
import {computed,onBeforeUnmount,onMounted,ref,watch} from 'vue'
import {Plus,Search,Folder,ChevronRight,Copy,Clock3,FileText,Tag,Lightbulb,Play,RotateCw,MoreHorizontal,MessageCircleMore} from 'lucide-vue-next'
import PageHeader from '../components/PageHeader.vue'
import ProviderIcon from '../components/ProviderIcon.vue'
import TaskModal from '../components/TaskModal.vue'
import Select from '../components/Select.vue'
import {api,call,desktop,formatDate,providerName} from '../services/api'
import {refreshSessionLocks} from '../services/session-locks'
import {store,action} from '../stores/store'
import {cachedSessions,readSessions,sessionCacheFresh,sessionView} from '../stores/sessionCache'
import type {Conversation,ConversationDetail,ProviderId,Task} from '../types'

const providerOptions = computed(()=>[
  { value: 'all', label: tr("全部提供商") },
  { value: 'codex', label: 'Codex' },
  { value: 'claude_code', label: 'Claude Code' },
] as const)
const projectOptions = computed(() => [
  { value: 'all', label: tr("全部项目") },
  ...projects.value.map(p => ({ value: p, label: p }))
])
const rootOptions=computed(()=>[...new Set([...(store.settings?.authorized_roots||[]),...projects.value])].map(value=>({value,label:value})))
const initial=cachedSessions(sessionView.provider,sessionView.search)
const items=ref<Conversation[]>(initial?.data||[]),search=ref(sessionView.search),provider=ref(sessionView.provider),project=ref(sessionView.project),detailTab=ref('overview'),selectedId=ref(sessionView.selectedId),selectedRows=ref<string[]>([...sessionView.selectedRows]),detail=ref<ConversationDetail>(),detailError=ref(''),busy=ref(false),errors=ref<string[]>(initial?.errors||[]),editing=ref(false),immediate=ref(false),editTask=ref<Task>(),newOpen=ref(false),newProvider=ref<ProviderId>('codex'),newRoot=ref('')
const page=ref(sessionView.page),pageSize=ref(sessionView.pageSize),complete=ref(initial?.complete||false),hasSnapshot=ref(!!initial)
let generation=0,detailGeneration=0,debounce:ReturnType<typeof setTimeout>
const key=(c:Conversation)=>c.provider+':'+c.session_id
const projects=computed(()=>[...new Set(items.value.map(c=>c.cwd))])
const detailConversation=ref<Conversation>(),taskConversation=ref<Conversation>(),menu=ref<{conversation:Conversation;x:number;y:number;id:number}>()
let menuId=0
const related=(c:Conversation)=>store.tasks.filter(t=>t.request.conversation.provider===c.provider&&t.request.conversation.session_id===c.session_id&&t.state!=='cancelled')
function status(c:Conversation){const jobs=related(c);return jobs.some(t=>['running','claimed'].includes(t.state))?'进行中':c.lock_state==='in_use'?'使用中':c.lock_state==='unknown'?'占用状态未知':jobs.some(t=>t.state==='paused')?'已暂停':c.resumable?'可继续':'不可恢复'}
const filtered=computed(()=>items.value.filter(c=>(provider.value==='all'||c.provider===provider.value)&&(project.value==='all'||c.cwd===project.value)&&[c.title,c.cwd].some(v=>v.toLowerCase().includes(search.value.toLowerCase()))).sort((a,b)=>(b.last_updated_utc||'').localeCompare(a.last_updated_utc||'')))
const selected=computed(()=>filtered.value.find(c=>key(c)===selectedId.value)||filtered.value[0])
const totalPages=computed(()=>Math.max(1,Math.ceil(filtered.value.length/pageSize.value)))
const pageRows=computed(()=>filtered.value.slice((page.value-1)*pageSize.value,page.value*pageSize.value))
const selectionCount=computed(()=>filtered.value.filter(c=>selectedRows.value.includes(key(c))).length)
const pageSelected=computed(()=>!!pageRows.value.length&&pageRows.value.every(c=>selectedRows.value.includes(key(c))))
const pagePartial=computed(()=>!pageSelected.value&&pageRows.value.some(c=>selectedRows.value.includes(key(c))))
watch([search,provider,project,pageSize],()=>{page.value=1;menu.value=undefined},{flush:'sync'})
watch(totalPages,count=>{page.value=Math.min(page.value,count)})
function selectPage(){const ids=new Set(selectedRows.value);for(const c of pageRows.value){if(pageSelected.value)ids.delete(key(c));else ids.add(key(c))}selectedRows.value=[...ids]}
async function load(force=false){
 clearTimeout(debounce);if(!desktop)return
 const version=++generation,query=search.value,id=provider.value,cached=cachedSessions(id,query)
 if(cached){items.value=cached.data;complete.value=cached.complete;errors.value=cached.errors;hasSnapshot.value=true}
 if(!force&&sessionCacheFresh(cached)){busy.value=false;return}
 busy.value=true
 try{
  const snapshot=await readSessions(id,query,force)
  if(version!==generation)return
  items.value=snapshot.data;complete.value=snapshot.complete;errors.value=snapshot.errors;hasSnapshot.value=true
  if(snapshot.complete&&!query&&id==='all'){const seen=new Set(items.value.map(key));selectedRows.value=selectedRows.value.filter(id=>seen.has(id))}
 }finally{if(version===generation)busy.value=false}
}
watch([search,provider],()=>{generation++;complete.value=false;hasSnapshot.value=false;if(desktop)busy.value=true;clearTimeout(debounce);debounce=setTimeout(()=>void load(),350)},{flush:'sync'})
watch(detailConversation,async c=>{detail.value=undefined;detailError.value='';const version=++detailGeneration;if(!c)return;try{const result=await api.detail(c);if(version===detailGeneration)detail.value=result}catch(e){if(version===detailGeneration)detailError.value=String(e)}})
let lockRefresh:ReturnType<typeof setInterval>
onMounted(()=>{void load();void refreshSessionLocks(pageRows.value);lockRefresh=setInterval(()=>{if(!document.hidden)void refreshSessionLocks(pageRows.value)},5000)})
watch(pageRows,rows=>{if(rows.length)void refreshSessionLocks(rows)})
function toggle(c:Conversation){const id=key(c);selectedRows.value=selectedRows.value.includes(id)?selectedRows.value.filter(v=>v!==id):[...selectedRows.value,id];selectedId.value=id}
function schedule(c:Conversation,now=false){taskConversation.value=c;editTask.value=undefined;immediate.value=now;detailConversation.value=undefined;editing.value=true}
function openMenu(event:MouseEvent|KeyboardEvent,c:Conversation){
 const target=event.currentTarget as HTMLElement,rect=target.getBoundingClientRect()
 target.focus();selectedId.value=key(c)
 const pointer=event instanceof MouseEvent&&(event.clientX!==0||event.clientY!==0)
 menu.value={conversation:c,x:pointer?event.clientX:rect.right,y:pointer?event.clientY:rect.bottom,id:++menuId}
}
function menuAction(action:'schedule'|'immediate'|'detail'){
 const c=menu.value?.conversation;menu.value=undefined;if(!c)return
 if(action==='detail'){detailTab.value='overview';detailConversation.value=c}else schedule(c,action==='immediate')
}
function editRelated(task:Task){editTask.value=task;immediate.value=false;detailConversation.value=undefined;editing.value=true}
onBeforeUnmount(()=>{clearInterval(lockRefresh);Object.assign(sessionView,{search:search.value,provider:provider.value,project:project.value,page:page.value,pageSize:pageSize.value,selectedId:selectedId.value,selectedRows:[...selectedRows.value]});generation++;detailGeneration++;clearTimeout(debounce)})
async function launch(){await action(()=>call('open_provider',{provider:newProvider.value,cwd:newRoot.value}),'官方工具已打开，创建会话后请刷新列表');newOpen.value=false}
async function copy(value:string){await action(()=>navigator.clipboard.writeText(value),'已复制')}
const summary=computed(()=>detail.value?.messages.filter(m=>m.role==='assistant').at(-1)?.text||detail.value?.messages.at(-1)?.text)
</script>
<template><div class="session-page"><PageHeader :title="tr('会话记录')" :subtitle="tr('选择已有对话，用于自动继续和恢复执行')"><button class="primary new-button" @click="newOpen=true;newRoot=selected?.cwd||store.settings?.authorized_roots[0]||''"><Plus/>{{tr("新建会话")}}</button></PageHeader><div class="session-toolbar"><div class="input-icon"><Search/><input v-model="search" :aria-label="tr('搜索会话')" :placeholder="tr('搜索会话标题、项目路径或内容…')"></div><Select v-model="provider" :options="providerOptions" :aria-label="tr('会话提供商')" :placeholder="tr('选择提供商')" /><div class="project-filter"><Folder :size="19"/><Select v-model="project" :options="projectOptions" :aria-label="tr('会话项目')" :placeholder="tr('选择项目')" /></div><button class="icon-button" :title="tr('刷新会话')" :disabled="busy" @click="load(true)"><RotateCw :class="{spin:busy}"/></button></div><p v-for="error in errors" :key="error" class="field-message">{{tr(error)}}</p><div class="sessions-layout"><section class="panel session-table-panel"><div class="table-scroll"><table class="sessions-table"><thead><tr><th><input type="checkbox" :aria-label="tr('选择全部会话')" :checked="pageSelected" :indeterminate="pagePartial" @change="selectPage"></th><th>{{tr("会话标题")}}</th><th>{{tr("提供商")}}</th><th>{{tr("项目路径")}}</th><th>{{tr("最后更新")}}</th><th>{{tr("状态")}}</th><th/></tr></thead><tbody><tr v-for="c in pageRows" :key="key(c)" :class="{selected:selected?.session_id===c.session_id&&selected.provider===c.provider}" tabindex="0" @click="selectedId=key(c)" @contextmenu.prevent="openMenu($event,c)" @keydown.enter.self.prevent="openMenu($event,c)" @keydown.shift.f10.prevent="openMenu($event,c)" @keydown.context-menu.prevent="openMenu($event,c)"><td @click.stop><input type="checkbox" :aria-label="tr('选择 ')+c.title" :checked="selectedRows.includes(key(c))" @change="toggle(c)"></td><td><strong :title="c.title">{{c.title}}</strong></td><td><span class="table-provider"><ProviderIcon :provider="c.provider" size="small"/>{{providerName(c.provider)}}</span></td><td><span class="path-ellipsis" :title="c.cwd">{{c.cwd}}</span></td><td>{{formatDate(c.last_updated_utc)}}</td><td><span class="badge" :class="{scheduled:c.resumable,paused:status(c)==='已暂停',in_use:status(c)==='使用中',lock_unknown:status(c)==='占用状态未知'}"><i class="status-dot"/>{{tr(status(c))}}</span></td><td><button class="icon-button session-menu-trigger" :aria-label="tr('会话操作：')+c.title" aria-haspopup="menu" @click.stop="openMenu($event,c)"><MoreHorizontal :size="18"/></button></td></tr></tbody></table></div><div v-if="!filtered.length" class="empty-state"><MessageCircleMore/><h2>{{busy&&!hasSnapshot?tr("正在读取已有会话…"):tr("没有匹配的会话")}}</h2><p>{{desktop?tr("请在官方工具中创建会话，或调整筛选条件。"):tr("浏览器预览未连接桌面服务，真实会话将在桌面应用中读取。")}}</p><button @click="newOpen=true">{{tr("新建会话")}}</button></div><PaginationBar class="session-pagination" subject="sessions" :total="filtered.length" :selected="selectionCount" :busy="busy&&!hasSnapshot" :partial="!complete" v-model:page="page" v-model:page-size="pageSize"><template #summary-actions><button v-if="selectionCount" class="text-button clear-session-selection" @click="selectedRows=[]">{{tr('清空选择')}}</button></template></PaginationBar></section>
</div>
<SessionContextMenu v-if="menu" :key="menu.id" :x="menu.x" :y="menu.y" :resumable="menu.conversation.resumable" @close="menu=undefined" @action="menuAction"/><AppDialog v-if="detailConversation" :title="tr('会话详情')" :description="detailConversation.title" wide panel-class="session-details-dialog" @close="detailConversation=undefined"><div class="session-detail"><div class="detail-heading"><ProviderIcon :provider="detailConversation.provider"/><div><h3>{{detailConversation.title}}</h3><span class="session-provider"><ProviderIcon :provider="detailConversation.provider" size="small"/>{{providerName(detailConversation.provider)}}<span class="badge scheduled" :class="{in_use:status(detailConversation)==='使用中',lock_unknown:status(detailConversation)==='占用状态未知'}"><i class="status-dot"/>{{tr(status(detailConversation))}}</span></span></div></div><div class="detail-tabs"><button v-for="(label,id) in {overview:'概览',content:'对话内容',tasks:'关联任务',settings:'设置'}" :key="id" :class="{active:detailTab===id}" @click="detailTab=id">{{tr(label)}}</button></div><template v-if="detailTab==='overview'"><section class="detail-section session-info"><h3>{{tr("基本信息")}}</h3><dl><dt><Folder/>{{tr("项目路径")}}</dt><dd :title="detailConversation.cwd">{{detailConversation.cwd}}<button class="icon-button" :title="tr('复制项目路径')" @click="copy(detailConversation.cwd)"><Copy :size="16"/></button></dd><dt><Clock3/>{{tr("创建时间")}}</dt><dd>{{formatDate(detail?.created_at||null)}}</dd><dt><Clock3/>{{tr("最后更新")}}</dt><dd>{{formatDate(detailConversation.last_updated_utc)}}</dd><dt><FileText/>{{tr("消息数量")}}</dt><dd>{{detail?.message_count??tr("未知")}}{{detail?.message_count!==null&&detail?.message_count!==undefined?tr(" 条"):''}}</dd><dt><Tag/>{{tr("涉及文件")}}</dt><dd>{{detail?.file_count??tr("未知")}}</dd></dl></section><section class="detail-section message-summary"><h3>{{tr("最新消息摘要")}}</h3><p>{{summary?.slice(0,220)||(detailError?tr("无法读取对话内容，请检查原会话。"):detail?tr("暂无可读取的消息。"):tr("正在读取会话详情…"))}}</p></section><section class="detail-section next-suggestion"><h3>{{tr("推荐下一步")}} <small>{{tr("本地提示")}}</small></h3><p><Lightbulb :size="19"/>{{detailConversation.resumable?tr("可继续当前会话，或创建定时任务，在指定时间恢复执行。"):tr("项目目录或会话已不可恢复，请在官方工具中检查。")}}</p></section></template><template v-else-if="detailTab==='content'"><div class="conversation-content"><p v-if="detailError" class="field-message">{{tr(detailError)}}</p><p v-if="detail?.truncated" class="muted">{{tr("仅展示已读取的部分内容，最多最近 200 条消息。")}}</p><article v-for="(message,index) in detail?.messages" :key="index" :class="message.role"><strong>{{message.role==='user'?tr("你"):tr("AI 助手")}}</strong><p>{{message.text}}</p></article><p v-if="detail&&!detail.messages.length" class="muted">{{tr("暂无可读取的对话消息。")}}</p></div></template><template v-else-if="detailTab==='tasks'"><div class="related-tasks"><button v-for="task in related(detailConversation)" :key="task.id" @click="editRelated(task)"><Clock3/><span>{{task.request.title}}<small>{{formatDate(task.request.scheduled_at_utc)}}</small></span><ChevronRight/></button><p v-if="!related(detailConversation).length" class="muted">{{tr("此会话尚无关联任务。")}}</p></div></template><template v-else><div class="detail-section session-settings"><h3>{{tr("会话信息")}}</h3><label>{{tr("会话 ID")}}</label><code>{{detailConversation.session_id}}</code><button @click="copy(detailConversation.session_id)"><Copy :size="16"/>{{tr("复制会话 ID")}}</button><label>{{tr("来源")}}</label><p>{{detailConversation.source}}</p><label>{{tr("恢复能力")}}</label><p>{{detailConversation.resumable?tr("可继续；执行前将重新验证"):tr("不可恢复，不能创建续聊任务")}}</p></div></template></div><template #footer><button class="primary" :disabled="!detailConversation.resumable" @click="schedule(detailConversation,true)"><Play :size="18"/>{{tr("立即继续")}}</button><button :disabled="!detailConversation.resumable" @click="schedule(detailConversation)"><Clock3 :size="19"/>{{tr("创建定时任务")}}</button></template></AppDialog><TaskModal v-if="editing" :task="editTask" :conversation="editTask?undefined:taskConversation" :immediate="immediate" @close="editing=false" @saved="editing=false"/><AppDialog v-if="newOpen" :title="tr('新建会话')" :description="tr('选择工具与项目，开始一段新的对话')" :close-label="tr('关闭新建会话')" @close="newOpen=false"><template #icon><MessageCircleMore :size="24"/></template><label class="form-label">{{tr("AI 提供商")}}</label><div class="provider-choice"><button type="button" :class="{selected:newProvider==='codex'}" :aria-pressed="newProvider==='codex'" @click="newProvider='codex'"><ProviderIcon provider="codex" size="small"/>Codex</button><button type="button" :class="{selected:newProvider==='claude_code'}" :aria-pressed="newProvider==='claude_code'" @click="newProvider='claude_code'"><ProviderIcon provider="claude_code" size="small"/>Claude Code</button></div><label for="new-session-root" class="form-label">{{tr("项目目录")}}</label><input id="new-session-root" v-model="newRoot" class="session-project-path" :aria-label="tr('项目目录')" :placeholder="tr('输入项目目录，例如 D:\\projects\\my-project')"><Select v-if="rootOptions.length" v-model="newRoot" :options="rootOptions" :placeholder="tr('从已有项目选择（可选）')" :aria-label="tr('常用项目')" /><p class="dialog-info"><Lightbulb :size="18"/>{{tr("将在官方工具中创建会话，完成后返回此页刷新列表。打开工具不会自动发送 AI 请求。")}}</p><template #footer><button @click="newOpen=false">{{tr("取消")}}</button><button class="primary" :disabled="!desktop||!newRoot" @click="launch">{{tr("打开")}} {{providerName(newProvider)}}<ChevronRight :size="16"/></button></template></AppDialog></div></template>

<style scoped>
.session-project-path{width:100%;margin-bottom:10px}
.session-page{min-height:calc(100dvh - var(--titlebar-height) - 48px);display:flex;flex-direction:column}
.session-page>.page-heading{min-height:60px;margin-bottom:16px;flex-shrink:0}
.session-page :deep(.page-heading h1){font-size:28px}
.session-page :deep(.page-heading p){font-size:13px;margin-top:5px}
.session-toolbar{flex-shrink:0;margin-bottom:16px;gap:9px}
.session-toolbar .input-icon{height:40px}
.session-toolbar .input-icon input{font-size:12px;padding:10px 8px}
.session-toolbar .input-icon>svg{width:17px;height:17px}
.sessions-layout{display:flex;flex:1;min-height:180px;align-items:stretch}
.session-table-panel{padding:0;min-height:180px;max-height:none;flex:1;display:flex;flex-direction:column;border-radius:12px}
.session-table-panel>.table-scroll{flex:0 0 auto;overflow:visible}
.sessions-table{table-layout:fixed}
.sessions-table th{background:var(--tint);font-size:12px;padding:13px 8px}
.sessions-table td{height:44px;padding:7px 8px;font-size:12px}
.sessions-table th:last-child,.sessions-table td:last-child{width:46px;padding-left:3px;padding-right:7px;text-align:center}
.sessions-table th:nth-child(3){width:155px}
.sessions-table th:nth-child(4){width:23%}
.sessions-table th:nth-child(5){width:135px}
.sessions-table th:nth-child(6){width:95px}
.sessions-table td:nth-child(2) strong{font-size:14px;max-width:100%}
.sessions-table .path-ellipsis{max-width:100%}
.clear-session-selection{font-size:11px;white-space:nowrap}
.session-pagination{margin-top:auto}
@media(max-width:1100px){.sessions-table th:nth-child(3){width:120px}.sessions-table th:nth-child(5){width:115px}.sessions-table th:nth-child(6){width:85px}}
@media(max-width:640px){.session-page :deep(.page-heading h1){font-size:25px}.session-toolbar{flex-wrap:wrap}.sessions-table th:nth-child(3){width:115px}.sessions-table th:nth-child(6){width:85px}}

.session-menu-trigger{padding:4px;min-width:28px}
.session-detail{padding:24px 28px;border:0;background:transparent;box-shadow:none}
.session-detail .detail-heading{margin-bottom:18px}
.session-detail .detail-heading h3{overflow-wrap:anywhere}
.session-detail .detail-tabs{justify-content:flex-start;gap:20px}
.session-detail .conversation-content,.session-detail .related-tasks{height:auto;min-height:240px;max-height:50dvh}
.session-detail .session-settings{min-height:240px}
.session-detail code{overflow-wrap:anywhere}
@media(max-width:640px){.session-detail{padding:18px}.session-detail .detail-tabs{gap:8px}}
@media(min-width:641px){.sessions-table td:nth-child(4),.sessions-table th:nth-child(4){display:table-cell}.path-ellipsis{max-width:230px}}
</style>
