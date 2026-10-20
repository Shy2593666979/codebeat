<script setup lang="ts">
import {tr} from '../composables/i18n'
import {ref,watch,onMounted,onUnmounted,nextTick} from 'vue'
import {listen,type UnlistenFn} from '@tauri-apps/api/event'
import {call,desktop,providerName} from '../services/api'
import type {InboxNotice} from '../types'
import {Check,X,Bell} from 'lucide-vue-next'
import {store} from '../stores/store'
type Notice={id:number;version:number;topic?:string;kind:'success'|'error'|'info';message:string;seen:boolean;remoteId?:string;remote?:boolean;title?:string}
const notices=ref<Notice[]>([]),pending:Notice[]=[],timers=new Map<number,ReturnType<typeof setTimeout>>()
const delivered=new Set<string>(),known=new Set<string>(),acknowledged=new Set<string>(),acking=new Set<string>(),publishing=new Map<string,Promise<unknown>>(),unlisten:UnlistenFn[]=[]
const topicFlights=new Map<string,Promise<unknown>>()
let nextId=0,closed=false,engagementTimer:ReturnType<typeof setInterval>
function drain(){while(notices.value.length<3&&pending.length)notices.value.push(pending.shift()!)}
function replaceOrQueue(next:Omit<Notice,'id'|'version'>){
 const current=next.topic?[...notices.value,...pending].find(n=>n.topic===next.topic):undefined
 if(current){clearTimeout(timers.get(current.id));timers.delete(current.id);Object.assign(current,next,{version:current.version+1,seen:false});return current}
 const notice:Notice={...next,id:++nextId,version:0};pending.push(notice);drain();return notice
}
function push(message:string,kind:'success'|'error',scope=''){
 if(!message.trim())return
 const topic=scope||`feedback:${kind}:${message}`,remoteId=desktop?crypto.randomUUID():undefined
 const provider=scope.startsWith('provider:')?scope.split(':')[1]:undefined
 const title=provider==='codex'||provider==='claude_code'?providerName(provider)+' · '+tr('自动继续'):tr(kind==='success'?'操作成功':'操作未完成')
 replaceOrQueue({topic,kind,message,title,seen:false,remoteId,remote:false})
 if(remoteId){
  known.add(remoteId)
  const flight=(topicFlights.get(topic)||Promise.resolve()).catch(()=>undefined).then(()=>call('publish_notice',{id:remoteId,topic,title,message,kind})).then(()=>{delivered.add(remoteId)}).catch(()=>undefined)
  topicFlights.set(topic,flight);publishing.set(remoteId,flight)
  void flight.finally(()=>{if(topicFlights.get(topic)===flight)topicFlights.delete(topic)})
 }
}
function acknowledgeRemote(notice:Notice){
 const id=notice.remoteId;if(!id||acknowledged.has(id)||acking.has(id))return
 acking.add(id)
 void (publishing.get(id)||Promise.resolve()).then(()=>call('acknowledge_notices',{ids:[id]})).then(()=>{acknowledged.add(id);publishing.delete(id)}).catch(()=>undefined).finally(()=>acking.delete(id))
}
function fullyVisible(element:HTMLElement){
 if(document.hidden||!document.hasFocus()||document.fonts.status!=='loaded')return false
 const box=element.getBoundingClientRect(),container=element.parentElement!.getBoundingClientRect()
 if(box.width===0||box.height===0||box.left<Math.max(0,container.left)||box.top<Math.max(0,container.top)||box.right>Math.min(innerWidth,container.right)||box.bottom>Math.min(innerHeight,container.bottom))return false
 const visible=document.elementFromPoint(box.left+box.width/2,box.top+box.height/2)
 return !!visible&&element.contains(visible)
}
async function canPresent(){if(!desktop)return true;try{return await call<boolean>('can_present_notices')}catch{return false}}
async function markSeen(notice:Notice,element:HTMLElement){
 const version=notice.version
 if(notice.seen||!fullyVisible(element)||!await canPresent()||version!==notice.version||closed||notice.seen||!fullyVisible(element))return
 notice.seen=true;acknowledgeRemote(notice)
 timers.set(notice.id,setTimeout(()=>dismiss(notice.id,version),2000))
}
function acknowledge(notice:Notice,event:Event){markSeen(notice,event.currentTarget as HTMLElement)}
async function checkEngagement(){
 if(document.fonts.status==='loading'){void document.fonts.ready.then(()=>{if(!closed)checkEngagement()});return}
 if(!await canPresent()||closed)return
 for(const notice of notices.value){const element=document.querySelector<HTMLElement>('[data-feedback-id="'+notice.id+'"]');if(!element||!fullyVisible(element))continue
  acknowledgeRemote(notice)
  if(notice.remote||element.matches(':hover')||element.contains(document.activeElement))markSeen(notice,element)
 }
}
function dismiss(id:number,version?:number){const notice=notices.value.find(n=>n.id===id);if(version!==undefined&&notice?.version!==version)return;if(notice)acknowledgeRemote(notice);clearTimeout(timers.get(id));timers.delete(id);notices.value=notices.value.filter(n=>n.id!==id);drain()}
function receive(items:InboxNotice[]){
 if(closed)return
 const ids=new Set(items.map(item=>item.id)),topics=new Set(items.map(item=>item.topic).filter(Boolean))
 // 从托盘忽略只移除排队中的卡片；已经显示的卡片仍保留两秒倒计时。
 const retained=(n:Notice)=>n.seen||!!n.topic&&topics.has(n.topic)||!n.remoteId||ids.has(n.remoteId)||acking.has(n.remoteId)||acknowledged.has(n.remoteId)||!n.remote&&!delivered.has(n.remoteId)
 notices.value=notices.value.filter(retained)
 for(let i=pending.length-1;i>=0;i--)if(!retained(pending[i]))pending.splice(i,1)
 for(const item of items){if(known.has(item.id))continue;known.add(item.id);replaceOrQueue({topic:item.topic||undefined,kind:item.kind,message:item.message,title:item.title,remoteId:item.id,remote:true,seen:false})}
 drain();void nextTick(checkEngagement)
}
async function reloadInbox(){if(desktop)try{receive(await call<InboxNotice[]>('get_pending_notices'))}catch{/* Existing local feedback remains available. */}}
watch(()=>store.notice,message=>{if(message){push(message,'success',store.noticeTopic);store.notice='';store.noticeTopic=''}},{immediate:true})
watch(()=>store.error,message=>{if(message){push(message,'error',store.errorTopic);store.error='';store.errorTopic=''}},{immediate:true})
onMounted(async()=>{
 window.addEventListener('focus',checkEngagement);document.addEventListener('visibilitychange',checkEngagement);window.addEventListener('resize',checkEngagement);engagementTimer=setInterval(checkEngagement,1000)
 if(desktop){for(const [name,handler] of [['notices-changed',(event:{payload:InboxNotice[]})=>receive(event.payload)],['inbox-present',()=>void reloadInbox()]] as const){const stop=await listen(name,handler);if(closed)stop();else unlisten.push(stop)}await reloadInbox()}
})
onUnmounted(()=>{closed=true;for(const timer of timers.values())clearTimeout(timer);clearInterval(engagementTimer);unlisten.forEach(stop=>stop());window.removeEventListener('focus',checkEngagement);document.removeEventListener('visibilitychange',checkEngagement);window.removeEventListener('resize',checkEngagement)})
</script>
<template><Teleport to="body"><section class="feedback-notices" :aria-label="tr('消息通知')" @scroll="checkEngagement"><TransitionGroup name="feedback" @after-enter="checkEngagement"><article v-for="notice in notices" :key="notice.id" :data-feedback-id="notice.id" class="feedback-card" :class="notice.kind" :role="notice.kind==='error'?'alert':'status'" @mouseenter="acknowledge(notice,$event)" @mousemove="acknowledge(notice,$event)" @focusin="acknowledge(notice,$event)" @keydown.esc="dismiss(notice.id)"><span class="feedback-state"><Check v-if="notice.kind==='success'"/><X v-else-if="notice.kind==='error'"/><Bell v-else/></span><div class="feedback-content"><h2>{{notice.title?tr(notice.title):notice.kind==='success'?tr("操作成功"):tr("操作未完成")}}</h2><p>{{tr(notice.message)}}</p><footer><span class="feedback-spacer" aria-hidden="true"></span><button class="primary" @click="dismiss(notice.id)">{{tr("知道了")}}</button><button class="feedback-dismiss" @click="dismiss(notice.id)">{{tr("关闭")}}</button></footer></div><button class="feedback-close" :aria-label="tr('关闭通知')" @click="dismiss(notice.id)"><X/></button><div v-if="notice.seen" :key="notice.version" class="feedback-countdown" aria-hidden="true"><span/></div></article></TransitionGroup></section></Teleport></template>
<style scoped>
.feedback-countdown{position:absolute;left:12px;right:12px;bottom:5px;height:3px;border-radius:2px;background:#0078ff18;overflow:hidden;pointer-events:none}.feedback-countdown span{display:block;width:100%;height:100%;border-radius:inherit;background:#0078ff;transform-origin:left;animation:feedback-countdown 2s linear forwards}.error .feedback-countdown{background:#f05b6a18}.error .feedback-countdown span{background:#f05b6a}@keyframes feedback-countdown{from{transform:scaleX(1)}to{transform:scaleX(0)}}

.feedback-notices{position:fixed;z-index:2000;top:44px;right:24px;width:min(500px,calc(100vw - 32px));display:flex;flex-direction:column;gap:16px;pointer-events:none;max-height:calc(100dvh - 60px);overflow:auto;scrollbar-width:thin;padding:0 0 12px 12px;box-sizing:content-box}
.feedback-card{pointer-events:auto;display:flex;gap:17px;position:relative;flex-shrink:0;min-height:176px;padding:19px 22px 14px 24px;border:1px solid #dbeafa;border-radius:14px;background:rgba(255,255,255,.99);box-shadow:0 4px 18px #6183ab33;color:#101635;font-family:"CodeBeat UI","Microsoft YaHei UI",sans-serif;box-sizing:border-box}
.feedback-state{display:flex;align-items:center;justify-content:center;width:46px;height:46px;border-radius:50%;background:linear-gradient(135deg,#00c7a0,#00ae87);flex-shrink:0}.feedback-state svg{width:28px;height:28px;color:white;stroke-width:3.3}.error .feedback-state{background:#f05b6a}.info .feedback-state{background:#1684ff}
.feedback-content{flex:1;min-width:0;display:flex;flex-direction:column}.feedback-content h2{font-size:20px;font-weight:600;line-height:30px;padding-right:25px;letter-spacing:0;margin:0}.feedback-content p{font-size:17px;line-height:27px;color:#657497;margin:8px 0 7px;overflow-wrap:anywhere;max-height:108px;overflow:auto;white-space:pre-wrap}.feedback-content footer{display:flex;align-items:center;gap:20px;margin-top:auto;min-height:40px}.feedback-spacer{flex:1}.feedback-content button{font-size:17px;font-weight:600;height:40px;min-height:40px;border-radius:10px;white-space:nowrap}.feedback-content .primary{width:116px;padding:0;background:linear-gradient(#287fff,#006dff);border:0;box-shadow:none;color:white}.feedback-dismiss{color:#0078ff;padding:0;border:0;background:transparent}.feedback-close{position:absolute;top:16px;right:15px;width:22px;height:22px;min-height:22px;padding:0;border:0;color:#68799b;background:transparent}.feedback-close svg{width:19px;height:19px;stroke-width:1.7}.feedback-enter-active,.feedback-leave-active{transition:opacity .16s,transform .16s}.feedback-enter-from,.feedback-leave-to{opacity:0;transform:translateX(16px)}
@media(prefers-reduced-motion:reduce){.feedback-enter-active,.feedback-leave-active{transition:none}}
@media(max-width:540px){.feedback-notices{right:12px;top:58px;width:calc(100vw - 36px)}.feedback-card{padding:16px;gap:12px}.feedback-content h2{font-size:18px}.feedback-content p{font-size:15px}.feedback-content footer{gap:14px}.feedback-content button{font-size:15px}.feedback-content .primary{width:92px}.feedback-state{width:38px;height:38px}}
</style>
