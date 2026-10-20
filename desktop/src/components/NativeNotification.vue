<script setup lang="ts">
import {ref,onMounted,onUnmounted,nextTick} from 'vue'
import {invoke} from '@tauri-apps/api/core'
import {Check,X,Info} from 'lucide-vue-next'

type Payload={title:string;body:string;kind:'success'|'error'|'info';page:'tasks'|'usage';task_id:string|null}
const payload=ref<Payload>(),seen=ref(false),error=ref(''),card=ref<HTMLElement>(),ready=ref(false)
let checking=false,closed=false
async function acknowledge(){
 if(checking||seen.value||!ready.value||document.hidden||document.fonts.status==='loading')return
 const box=card.value?.getBoundingClientRect()
 if(!box||!box.width||box.left<0||box.top<0||box.right>innerWidth||box.bottom>innerHeight)return
 checking=true
 try{seen.value=await invoke<boolean>('notification_seen')}catch(e){error.value=String(e)}finally{checking=false}
}
async function dismiss(open=false){
 try{await invoke('notification_close',{open})}catch(e){error.value=String(e)}
}
function escape(event:KeyboardEvent){if(event.key==='Escape')void dismiss()}
onMounted(async()=>{
 window.addEventListener('keydown',escape)
 try{
  payload.value=await invoke<Payload>('notification_payload')
  await nextTick();await document.fonts.ready
  // 等待浏览器完成布局和绘制准备，再由原生端显示窗口，避免闪出空白。
  await new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve())))
  if(closed)return
  await invoke('notification_ready');ready.value=true
 }catch(e){error.value=String(e)}
})
onUnmounted(()=>{closed=true;window.removeEventListener('keydown',escape)})
</script>
<template>
 <article v-if="payload" ref="card" class="native-notification" :class="payload.kind" :role="payload.kind==='error'?'alert':'status'" @mouseenter="acknowledge" @mousemove="acknowledge" @focusin="acknowledge">
  <span class="native-state"><Check v-if="payload.kind==='success'"/><X v-else-if="payload.kind==='error'"/><Info v-else/></span>
  <div class="native-content"><h2>{{payload.title}}</h2><p>{{payload.body}}</p><footer><button class="native-open" @click="dismiss(true)">{{payload.page==='usage'?'查看额度':'查看任务'}}</button><button class="native-dismiss" @click="dismiss()">关闭</button></footer></div>
  <button class="native-close" aria-label="关闭通知" @click="dismiss()"><X/></button>
  <div v-if="seen" class="native-countdown" aria-hidden="true"><span/></div>
 </article>
 <span v-if="error" class="native-error" role="alert">{{error}}</span>
</template>
<style>
html[data-surface="notification"],html[data-surface="notification"] body,html[data-surface="notification"] #app{background:transparent!important;margin:0!important;min-width:0!important;min-height:0!important;width:100%;height:100%;overflow:hidden;border:0;border-radius:0;box-shadow:none}
</style>
<style scoped>
.native-notification{position:relative;display:flex;gap:17px;box-sizing:border-box;width:500px;height:176px;margin:12px;padding:19px 22px 14px 24px;border:1px solid #dbeafa;border-radius:14px;background:rgba(255,255,255,.99);box-shadow:0 3px 16px #6183ab33;color:#101635;font-family:"CodeBeat UI","Microsoft YaHei UI",sans-serif}
.native-state{display:flex;align-items:center;justify-content:center;flex-shrink:0;width:46px;height:46px;border-radius:50%;background:linear-gradient(135deg,#00c7a0,#00ae87)}.native-state svg{width:27px;height:27px;color:white;stroke-width:3.3}.error .native-state{background:#f05b6a}.info .native-state{background:#1684ff}
.native-content{min-width:0;flex:1;display:flex;flex-direction:column}.native-content h2{font-size:20px;font-weight:600;line-height:34px;margin:0;padding-right:25px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;letter-spacing:0}.native-content p{font-size:17px;line-height:27px;color:#657497;margin:4px 0 7px;white-space:pre-wrap;overflow-wrap:anywhere;overflow:hidden;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical}.native-content footer{display:flex;justify-content:flex-end;align-items:center;gap:20px;margin-top:auto;height:40px;flex-shrink:0}.native-notification button{font:600 17px "CodeBeat UI",sans-serif;min-height:0;box-shadow:none;cursor:pointer;border:0}.native-open{height:40px;width:116px;padding:0;border-radius:10px;color:white;background:linear-gradient(#287fff,#006dff)}.native-dismiss{height:40px;width:46px;padding:0;color:#0078ff;background:transparent}.native-close{position:absolute;right:15px;top:16px;width:22px;height:22px;padding:0;color:#68799b;background:transparent}.native-close svg{width:19px;height:19px;stroke-width:1.7}.native-notification button:hover{opacity:.8}.native-notification button:focus-visible{outline:2px solid #79b8ff;outline-offset:2px}
.native-countdown{position:absolute;left:12px;right:12px;bottom:5px;height:3px;border-radius:2px;background:#0078ff18;overflow:hidden;pointer-events:none}.native-countdown span{display:block;height:100%;background:#0078ff;transform-origin:left;animation:native-countdown 2s linear forwards}.error .native-countdown{background:#f05b6a18}.error .native-countdown span{background:#f05b6a}@keyframes native-countdown{from{transform:scaleX(1)}to{transform:scaleX(0)}}.native-error{position:absolute;bottom:12px;left:24px;font-size:11px;color:#b42335}
</style>
