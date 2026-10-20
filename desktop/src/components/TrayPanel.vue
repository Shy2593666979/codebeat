<script setup lang="ts">
import {tr,language} from '../composables/i18n'
import {computed,nextTick,onMounted,onUnmounted,ref} from 'vue'
import {listen,type UnlistenFn} from '@tauri-apps/api/event'
import {Monitor,PlayCircle,PauseCircle,ChartNoAxesColumnIncreasing,Settings as SettingsIcon,LogOut,ChevronRight,Bell,CheckCircle2,AlertCircle,MessageSquareText,X} from 'lucide-vue-next'
import BrandMark from './BrandMark.vue'
import ProviderIcon from './ProviderIcon.vue'
import {call,desktop,providerName,remainingPercent} from '../services/api'
import {relativeTime} from '../services/presentation'
import type {Settings,Task,UsageReport,ProviderId,InboxNotice,UsageRefreshState} from '../types'
type Layout={opensLeft:boolean;tail:number;above:boolean;messages?:boolean;scale?:number}
type Snapshot={settings:Settings;tasks:Task[];usage:(UsageReport|null)[];usageRefresh?:UsageRefreshState[];layout?:Layout|null;inbox?:InboxNotice[]}
type Section='status'|'tasks'|'usage'|'settings'|null
const snapshot=ref<Snapshot>(),section=ref<Section>(null),busy=ref(false),error=ref(''),now=ref(Date.now())
const messages=ref(false),trayScale=ref(.85),stage=ref<HTMLElement|null>(null),submenuTop=ref(0),submenuTail=ref(24)
const opensLeft=ref(false),tail=ref(294),above=ref(true),menu=ref<HTMLElement|null>(null),submenu=ref<HTMLElement|null>(null)
let timer:ReturnType<typeof setInterval>,unlisten:UnlistenFn[]=[]
let destroyed=false
const paused=computed(()=>snapshot.value?.settings.paused??false)
const tasks=computed(()=>snapshot.value?.tasks.filter(t=>['scheduled','paused','running','claimed','needs_review'].includes(t.state)).sort((a,b)=>Date.parse(a.request.scheduled_at_utc)-Date.parse(b.request.scheduled_at_utc))??[])
const eligible=computed(()=>tasks.value.find(t=>t.state==='scheduled'&&!snapshot.value?.settings.paused_providers.includes(t.request.conversation.provider)))
const status=computed(()=>!snapshot.value?'正在读取运行状态':paused.value?'所有任务已暂停':eligible.value?tr('运行中 · 预计 {time}继续',{time:relativeTime(eligible.value.request.scheduled_at_utc,now.value)}):'运行中 · 暂无待执行任务')
const providers:ProviderId[]=['codex','claude_code']
function usage(id:ProviderId){return snapshot.value?.usage.find(r=>r?.provider===id)}
function quotaText(id:ProviderId){
 const report=usage(id),resets=report?.windows.map(w=>w.resets_at_utc).filter((t):t is string=>!!t&&Date.parse(t)>now.value).sort()
 const stale=snapshot.value?.usageRefresh?.some(s=>s.provider===id&&s.retrying)?tr('上次成功数据：'):''
 if(resets?.length)return stale+tr('额度将在 {time}恢复',{time:relativeTime(resets[0],now.value)})
 return report?.windows.length?stale+report.windows.map(w=>remainingPercent(w.used_percent)).filter((v):v is number=>v!==null).map(v=>tr('剩余 {percent}%',{percent:Math.round(v)})).join(' · ')||'剩余额度未知':'暂未读取额度'
}
function providerStatus(id:ProviderId){const count=tasks.value.filter(t=>t.request.conversation.provider===id&&['scheduled','paused'].includes(t.state)).length;return count?tr(paused.value||snapshot.value?.settings.paused_providers.includes(id)?'{count} 个任务等待恢复':'{count} 个任务等待执行',{count}):tr('暂无待执行任务')}
function applyLayout(layout:Layout){trayScale.value=layout.scale??.85;messages.value=!!layout.messages;opensLeft.value=layout.opensLeft;tail.value=layout.tail;above.value=layout.above}
async function reload(){try{snapshot.value=await call<Snapshot>('get_tray_snapshot');language.value=snapshot.value.settings.language;document.documentElement.lang=language.value;if(snapshot.value.layout)applyLayout(snapshot.value.layout);error.value='';now.value=Date.now();void nextTick(positionSubmenu)}catch(e){error.value=String(e)}}
async function act(action:string,taskId?:string){if(busy.value)return;busy.value=true;error.value='';try{await call('tray_action',{action,taskId:taskId??null});if(action==='pause')await reload()}catch(e){error.value=String(e)}finally{busy.value=false}}
async function ignoreAll(){if(busy.value)return;busy.value=true;try{await call('ignore_all_notices');await reload()}catch(e){error.value=String(e)}finally{busy.value=false}}
async function openNotice(id:string){try{await call('open_notice',{id})}catch(e){error.value=String(e)}}
function noticeTime(value:string){const date=new Date(value);if(!Number.isFinite(date.getTime()))return '';const today=new Date(now.value);return new Intl.DateTimeFormat(language.value,{...(date.toDateString()===today.toDateString()?{}:{month:'numeric',day:'numeric'}),hour:'2-digit',minute:'2-digit',hour12:false}).format(date)}
function positionSubmenu(){
 const row=stage.value?.querySelector<HTMLElement>('[data-tray-section="'+section.value+'"]')
 if(!row||!submenu.value||!stage.value)return
 const base=stage.value.getBoundingClientRect(),anchor=row.getBoundingClientRect(),flyout=submenu.value.getBoundingClientRect(),scale=trayScale.value
 const height=flyout.height/scale,available=base.height/scale
 submenuTop.value=Math.max(12,Math.min((anchor.top-base.top)/scale,available-height-12))
 submenuTail.value=Math.max(14,Math.min((anchor.top+anchor.height/2-base.top)/scale-submenuTop.value,height-14))
}
function openSection(value:Section){section.value=value;void nextTick(positionSubmenu)}
async function enterSection(value:Section){openSection(value);await nextTick();submenu.value?.querySelector<HTMLButtonElement>('button')?.focus()}
function navigate(event:KeyboardEvent){
 if(!['ArrowDown','ArrowUp','Home','End'].includes(event.key))return
 const root=(event.target as HTMLElement).closest('[role="menu"]');if(!root)return
 const items=Array.from(root.querySelectorAll<HTMLButtonElement>(':scope > button:not(:disabled),:scope > .tray-group > button:not(:disabled),:scope > .tray-submenu-content > button:not(:disabled),:scope > .inbox-items > button:not(:disabled),:scope > footer > button:not(:disabled)'))
 if(!items.length)return
 event.preventDefault();const index=items.indexOf(document.activeElement as HTMLButtonElement)
 items[event.key==='Home'?0:event.key==='End'?items.length-1:(index+(event.key==='ArrowUp'?-1:1)+items.length)%items.length]?.focus()
}
function escape(event:KeyboardEvent){event.preventDefault();if(messages.value){void act('hide');return;}if(section.value){section.value=null;menu.value?.querySelector<HTMLButtonElement>('.tray-row')?.focus()}else void act('hide')}
onMounted(async()=>{
 if(desktop){
  const track=(stop:UnlistenFn)=>{if(destroyed)stop();else unlisten.push(stop)}
  track(await listen<Layout>('tray-open',event=>{applyLayout(event.payload);section.value=null;void reload();if(!messages.value)void nextTick(()=>menu.value?.focus())}))
  track(await listen<InboxNotice[]>('notices-changed',event=>{if(snapshot.value)snapshot.value.inbox=event.payload}))
  await reload();if(!messages.value)await nextTick(()=>menu.value?.focus())
 }
 if(!destroyed)timer=setInterval(()=>{now.value=Date.now();if(messages.value||document.hasFocus())void reload()},3000)
})
onUnmounted(()=>{destroyed=true;clearInterval(timer);unlisten.forEach(stop=>stop())})
</script>

<template>
 <div ref="stage" class="tray-stage" :class="{'opens-left':opensLeft,'opens-below':!above,'messages-stage':messages}" :style="{'--tray-tail':tail+'px','--tray-scale':trayScale}" @pointerdown.self="act('hide')" @contextmenu.prevent @keydown="navigate" @keydown.esc="escape">
  <section v-if="messages" class="tray-menu tray-inbox" role="menu" :aria-label="tr('消息列表')">
   <header class="inbox-heading"><span class="inbox-heading-icon"><MessageSquareText :size="22"/></span><strong>{{tr('消息列表')}}</strong><span v-if="snapshot?.inbox?.length" class="inbox-count">{{snapshot.inbox.length}}</span><button class="inbox-close" :aria-label="tr('关闭')" @click="act('hide')"><X :size="19"/></button></header>
   <div class="inbox-items"><button v-for="notice in snapshot?.inbox||[]" :key="notice.topic||notice.id" class="inbox-item" :class="notice.kind" role="menuitem" @click="openNotice(notice.id)"><span class="inbox-type" aria-hidden="true"><CheckCircle2 v-if="notice.kind==='success'" :size="18"/><AlertCircle v-else-if="notice.kind==='error'" :size="18"/><Bell v-else :size="18"/></span><span class="inbox-copy"><span class="inbox-item-heading"><strong>{{tr(notice.title)}}</strong><time :datetime="notice.at">{{noticeTime(notice.at)}}</time></span><p>{{tr(notice.message)}}</p></span></button><div v-if="!snapshot?.inbox?.length" class="inbox-empty"><CheckCircle2 :size="28"/><p>{{tr('暂无新消息')}}</p></div></div>
   <footer><span class="inbox-footer-brand">CodeBeat</span><button class="ignore-all" role="menuitem" :disabled="busy||!snapshot?.inbox?.length" @click="ignoreAll">{{tr('忽略全部')}}</button></footer><p v-if="error" class="tray-error" role="alert">{{tr(error)}}</p>
  </section>
  <section v-else ref="menu" class="tray-menu" role="menu" :aria-label="tr('CodeBeat 托盘菜单')" tabindex="-1">
   <header class="tray-heading" @mouseenter="openSection(null)"><BrandMark/><div><h1>Code<span>Beat</span></h1><p><i :class="{paused}"/>{{tr(status)}}</p></div></header>
   <button class="tray-row home-row" role="menuitem" :disabled="busy" @mouseenter="openSection(null)" @click="act('open')"><svg class="home-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M3 10 12 2l9 8v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" fill="#087bff"/><rect x="9" y="13" width="6" height="6" rx=".7" fill="white"/></svg><span>{{tr("打开主界面")}}</span></button>
   <button class="tray-row" role="menuitem" aria-haspopup="menu" :aria-expanded="section==='status'" data-tray-section="status" @mouseenter="openSection('status')" @focus="openSection('status')" @keydown.right.prevent="enterSection('status')" @click="act('status')"><Monitor class="blue"/><span>{{tr("查看运行状态")}}</span></button>
   <button class="tray-row" role="menuitem" :disabled="busy||paused||!eligible" @mouseenter="openSection(null)" @click="act('continue',eligible?.id)"><PlayCircle class="green"/><span>{{tr("立即继续")}}</span></button>
   <button class="tray-row" role="menuitem" :disabled="busy||!snapshot" @mouseenter="openSection(null)" @click="act('pause')"><PlayCircle v-if="paused" class="green"/><PauseCircle v-else class="orange"/><span>{{paused?tr("恢复所有任务"):tr("暂停所有任务")}}</span></button>
   <div class="tray-group">
    <button class="tray-row" :class="{selected:section==='tasks'}" role="menuitem" aria-haspopup="menu" :aria-expanded="section==='tasks'" data-tray-section="tasks" @mouseenter="openSection('tasks')" @focus="openSection('tasks')" @keydown.right.prevent="enterSection('tasks')" @click="enterSection('tasks')"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><rect x="3" y="2" width="18" height="20" rx="3"/><path d="M8 7h8M8 12h8M8 17h4"/></svg><span>{{tr("任务列表")}}</span><ChevronRight class="chevron"/></button>
    <button class="tray-row" :class="{selected:section==='usage'}" role="menuitem" aria-haspopup="menu" :aria-expanded="section==='usage'" data-tray-section="usage" @mouseenter="openSection('usage')" @focus="openSection('usage')" @keydown.right.prevent="enterSection('usage')" @click="enterSection('usage')"><ChartNoAxesColumnIncreasing class="blue quota-icon"/><span>{{tr("额度概览")}}</span><ChevronRight class="chevron"/></button>
   </div>
   <div class="tray-group"><button class="tray-row" :class="{selected:section==='settings'}" role="menuitem" aria-haspopup="menu" :aria-expanded="section==='settings'" data-tray-section="settings" @mouseenter="openSection('settings')" @focus="openSection('settings')" @keydown.right.prevent="enterSection('settings')" @click="enterSection('settings')"><SettingsIcon/><span>{{tr("设置")}}</span><ChevronRight class="chevron"/></button></div>
   <div class="tray-group"><button class="tray-row exit-row" role="menuitem" :disabled="busy" @mouseenter="openSection(null)" @click="act('exit')"><LogOut/><span>{{tr("退出")}}</span></button></div>
   <p v-if="error" class="tray-error" role="alert">{{tr(error)}}</p>
  </section>
  <section v-if="!messages&&section" ref="submenu" class="tray-submenu" :style="{top:submenuTop+'px','--submenu-tail':submenuTail+'px'}" :class="'submenu-'+section" role="menu" :aria-label="section==='tasks'?tr('任务列表'):section==='settings'?tr('托盘设置'):tr('工具运行与额度')" @keydown.left.prevent="section=null;menu?.querySelector<HTMLButtonElement>('.tray-row')?.focus()"><div class="tray-submenu-content">
   <template v-if="section==='status'||section==='usage'">
    <button v-for="id in providers" :key="id" class="tray-provider" role="menuitem" @click="act('usage')"><ProviderIcon :provider="id"/><span><strong>{{providerName(id)}}</strong><small>{{tr(section==='usage'||id==='codex'?quotaText(id):providerStatus(id))}}</small></span></button>
   </template>
   <template v-else-if="section==='tasks'">
    <button v-for="task in tasks.slice(0,5)" :key="task.id" class="tray-task" role="menuitem" @click="act('tasks',task.id)"><ProviderIcon :provider="task.request.conversation.provider" size="small"/><span><strong>{{task.request.title}}</strong><small>{{task.state==='paused'?tr("任务已暂停"):task.state==='needs_review'?tr("等待确认"):relativeTime(task.request.scheduled_at_utc,now)}}</small></span></button>
    <p v-if="!tasks.length" class="tray-empty">{{tr("暂无待执行任务")}}</p><button class="tray-sub-action" role="menuitem" @click="act('tasks')">{{tr("查看全部任务")}}<ChevronRight :size="16"/></button>
   </template>
   <template v-else><button class="tray-sub-action" role="menuitem" @click="act('settings')">{{tr("打开设置")}}<ChevronRight :size="16"/></button><button class="tray-sub-action" role="menuitem" @click="act('usage')">{{tr("额度与连接状态")}}<ChevronRight :size="16"/></button></template>
  </div></section>
 </div>
</template>

<style>
html[data-surface=tray],html[data-surface=tray] body,html[data-surface=tray] #app{background:transparent!important;margin:0;width:100%;height:100%;min-width:0;overflow:hidden;color-scheme:light}
</style>

<style scoped>
.tray-stage{--tray-scale:.85;zoom:var(--tray-scale);width:calc(100vw / var(--tray-scale));height:calc(100dvh / var(--tray-scale));position:relative;color:#101635;font-family:"CodeBeat UI",sans-serif;user-select:none;--tray-line:#d9e8fc}
.tray-menu,.tray-submenu{position:absolute;background:linear-gradient(115deg,#fcfdfffc,#f4f9fffa);border:1px solid #fff;border-radius:13px;box-shadow:0 6px 24px #245d9c30;outline:none}
.tray-menu{left:14px;bottom:18px;width:326px;padding:8px;max-height:calc(100dvh / var(--tray-scale) - 36px);overflow:visible}
.tray-menu:after{content:"";position:absolute;bottom:-7px;left:calc(var(--tray-tail) - 7px);width:13px;height:13px;background:#f5faff;transform:rotate(45deg);border-radius:1px}
.tray-heading{height:92px;display:flex;align-items:center;gap:12px;padding:10px 8px 16px;border-bottom:1px solid var(--tray-line);margin:0 4px 8px}
.tray-heading>.brand-mark{width:47px;height:44px;flex-shrink:0}.tray-heading>div{min-width:0}.tray-heading h1{font-size:25px;line-height:34px;letter-spacing:-.8px;font-weight:700;margin:0;color:#101015}.tray-heading h1 span{color:#165cff}.tray-heading p{display:flex;align-items:flex-start;gap:5px;font-size:12px;line-height:20px;color:#657497;margin:2px 0 0;white-space:normal}.tray-heading i{width:10px;height:10px;background:#00be99;border-radius:50%;flex-shrink:0;margin-top:5px}.tray-heading i.paused{background:#ff8b2b}
.tray-row{display:flex;align-items:center;gap:23px;width:100%;height:52px;padding:0 16px;border:0;border-radius:9px;background:transparent;box-shadow:none;color:#101635;font-size:19px;font-weight:400;text-align:left;transition:background .12s;min-height:0}.tray-row>svg{width:27px;height:27px;flex-shrink:0;stroke-width:2}.tray-row>span{flex:1}.tray-row .chevron{width:17px;height:17px;color:#657497;margin-left:auto}.tray-row:hover,.tray-row.selected,.tray-row:focus-visible,.home-row{background:linear-gradient(110deg,#dceeff,#deecfd)}.tray-row:focus-visible,.tray-submenu button:focus-visible{outline:2px solid #0078ff;outline-offset:-2px}.tray-row:disabled{opacity:.45;cursor:default}.tray-row:active{transform:none}.tray-row .blue{color:#006dff}.tray-row .green{color:#00b992}.tray-row .orange{color:#ff790d}.tray-row .home-icon{fill:#087bff;stroke:white;paint-order:stroke;stroke-width:1.7}.tray-row .quota-icon{stroke-width:4}.exit-row>svg{color:#e32e0b}.tray-group{border-top:1px solid var(--tray-line);margin:8px 4px 0;padding-top:8px}.tray-group .tray-row{width:calc(100% + 8px);margin-left:-4px}.tray-error{font-size:12px;color:#c43d4a;line-height:1.5;margin:6px 8px;overflow-wrap:anywhere}
.tray-submenu{left:354px;width:238px;top:124px;padding:0 12px;overflow:visible}.tray-submenu-content{max-height:min(280px,calc(100dvh / var(--tray-scale) - 148px));overflow:auto;scrollbar-width:thin}.tray-submenu:before{content:"";position:absolute;left:-7px;top:calc(var(--submenu-tail,24px) - 6.5px);width:13px;height:13px;background:#f9fcff;transform:rotate(45deg)}.tray-provider{display:flex;align-items:flex-start;gap:11px;width:100%;padding:16px 0;border:0;border-radius:0;background:transparent;box-shadow:none;text-align:left;min-height:84px;color:#101635}.tray-provider+.tray-provider,.tray-task+.tray-task{border-top:1px solid var(--tray-line)}.tray-provider .provider-mark{width:37px;height:37px;border-radius:8px;flex-shrink:0}.tray-provider span,.tray-task span{min-width:0}.tray-provider strong{display:block;font-size:15px;font-weight:600;line-height:22px}.tray-provider small{display:block;font-size:14px;line-height:22px;color:#657497;margin-top:2px}.tray-task{width:100%;display:flex;align-items:center;gap:9px;padding:12px 0;border:0;border-radius:0;background:none;box-shadow:none;color:#101635;text-align:left}.tray-task strong{font-size:13px;font-weight:500;display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.tray-task small{font-size:12px;color:#657497;display:block;margin-top:3px}.tray-sub-action{display:flex;align-items:center;justify-content:space-between;width:100%;padding:14px 0;border:0;background:none;border-radius:0;box-shadow:none;font-size:14px;color:#101635}.tray-submenu button:hover{background:#dfedff80}.tray-empty{padding:16px 0;font-size:13px;color:#657497}.opens-left .tray-menu{left:266px}.opens-left .tray-submenu{left:14px}.opens-left .tray-submenu:before{left:auto;right:-7px}.opens-left .chevron{transform:rotate(180deg)}.opens-below .tray-menu{bottom:auto;top:18px}.opens-below .tray-menu:after{bottom:auto;top:-7px}

</style>

<style scoped>
/* 固定列宽让各个 provider/任务无论文字长短都能对齐。 */
.tray-submenu .tray-provider,.tray-submenu .tray-task{display:grid;grid-template-columns:32px minmax(0,1fr);column-gap:11px;justify-content:start;align-items:start;text-align:left}
.tray-submenu .tray-provider>.provider-mark,.tray-submenu .tray-task>.provider-mark{width:32px;height:32px;min-width:32px;justify-self:start}
.tray-submenu .tray-provider>span:last-child,.tray-submenu .tray-task>span:last-child{width:100%;min-width:0;text-align:left}
.tray-submenu .tray-provider small,.tray-submenu .tray-task small{overflow-wrap:anywhere}
</style>

<style scoped>
.messages-stage .tray-inbox{left:0;top:0;bottom:auto;width:354px;max-height:none;padding:0;overflow:visible;display:flex;flex-direction:column;background:linear-gradient(135deg,#fff,#f8fbff);isolation:isolate}
.tray-inbox:after{display:none;content:none}
.inbox-heading{height:56px;display:flex;gap:10px;align-items:center;padding:0 14px;font-size:17px;flex-shrink:0}
.inbox-heading-icon{width:32px;height:32px;display:grid;place-items:center;border-radius:10px;background:linear-gradient(135deg,#dceaff,#eff5ff);color:#087bff;flex-shrink:0}
.inbox-count{margin-left:auto;min-width:28px;height:28px;padding:0 7px;border-radius:9px;background:#e8f1ff;color:#087bff;font-size:14px;display:grid;place-items:center;font-weight:600}
.inbox-close{display:grid;place-items:center;width:29px;height:24px;min-height:0;border:0;border-left:1px solid #dce6f3;border-radius:0;background:none;box-shadow:none;padding:0 0 0 8px;color:#7e8a9e}.inbox-close:hover{color:#087bff}.inbox-close:focus-visible{outline:2px solid #087bff;outline-offset:2px}
.inbox-items{min-height:0;max-height:none;overflow:visible;padding:8px 10px 10px;display:flex;flex-direction:column;gap:10px}
.inbox-item{display:flex;align-items:flex-start;justify-content:start;text-align:left;gap:10px;flex-shrink:0;width:100%;height:104px;border:1px solid #e5eefb;border-radius:12px;background:#f5f9ff;padding:14px 11px;color:#17233b;min-height:0;box-shadow:none;transition:background .15s,border-color .15s}.inbox-item.success{background:#f5fbf8;border-color:#e3f1ec}.inbox-item.error{background:#fff9f8;border-color:#f5e9e6}.inbox-item:hover{background:#edf5ff;border-color:#cfe2fd}.inbox-item:active{transform:none}.inbox-item:focus-visible{outline:2px solid #087bff;outline-offset:-2px}
.inbox-type{width:40px;height:40px;display:grid;place-items:center;flex-shrink:0;border-radius:50%;background:#e6f0ff;color:#087bff;margin-top:2px}.inbox-type svg{width:22px;height:22px}.inbox-item.success .inbox-type{background:#e1f6ec;color:#00b67d}.inbox-item.error .inbox-type{background:#ffebe8;color:#da6b60}
.inbox-copy{min-width:0;flex:1}.inbox-item-heading{display:flex;align-items:baseline;gap:8px;min-width:0}.inbox-item strong{font-size:13px;font-weight:600;line-height:20px;flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.inbox-item time{font-size:10px;font-weight:400;line-height:16px;color:#94a0b2;flex-shrink:0}.inbox-item p{font-size:12px;line-height:20px;color:#8591a5;margin:6px 0 0;overflow-wrap:anywhere;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;max-height:40px}
.tray-inbox footer{height:44px;padding:0 13px 0 16px;display:flex;align-items:center;justify-content:space-between;border-top:1px solid #e1e9f5;flex-shrink:0}.inbox-footer-brand{font-size:11px;letter-spacing:.2px;color:#a2adbd}.ignore-all{border:0;background:transparent;box-shadow:none;border-radius:6px;font-size:13px;color:#087bff;min-height:28px;padding:4px 7px}.ignore-all:hover:enabled{background:#eaf2ff}.ignore-all:focus-visible{outline:2px solid #087bff}.ignore-all:disabled{opacity:.45}.inbox-empty{height:104px;display:grid;place-content:center;justify-items:center;gap:10px;color:#b1c1d5}.inbox-empty p{font-size:12px;color:#94a0b2;margin:0}.tray-inbox .tray-error{margin:0;padding:0 14px 10px;font-size:11px}

</style>

<style scoped>
.tray-menu,.tray-submenu{border:1px solid #e0e8f2;border-radius:15px;box-shadow:0 8px 30px #172f501c,0 2px 6px #172f500a;background:linear-gradient(145deg,#fff,#f8fbff)}.tray-heading{height:82px;gap:11px;padding-bottom:12px;margin-bottom:7px;border-bottom-color:#e5ecf5}.tray-heading>.brand-mark{width:40px;height:40px}.tray-heading h1{font-size:23px;letter-spacing:-.6px;line-height:30px}.tray-heading p{font-size:11px;line-height:18px;color:#8290a6}.tray-heading i{width:7px;height:7px;margin-top:5px}.tray-row{height:48px;font-size:17px;gap:19px;padding:0 15px;border-radius:9px}.tray-row>svg{width:24px;height:24px;stroke-width:1.8}.home-row{background:#edf4ff;color:#2879e8}.tray-row:hover,.tray-row.selected,.tray-row:focus-visible{background:#eaf2fd}.tray-row .quota-icon{stroke-width:3}.tray-group{border-top-color:#e5ecf5;margin-top:7px;padding-top:7px}.exit-row{color:#7b879a}.exit-row>svg{color:#c47870}.tray-provider{padding:14px 0}.tray-provider strong{font-size:14px}.tray-provider small{font-size:12px;line-height:20px;color:#7d8aa0}.tray-provider .provider-mark{width:32px;height:32px}.tray-inbox{background:#f9fbfe}
</style>
