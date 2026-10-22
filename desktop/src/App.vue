<script setup lang="ts">
import {tr} from './composables/i18n'
import {onMounted,onUnmounted,ref,watch} from 'vue'
import {useRouter} from 'vue-router'
import {House,CalendarCheck,ChartPie,Settings,Minus,Square,X} from 'lucide-vue-next'
import {getCurrentWindow} from '@tauri-apps/api/window'
import {listen,type UnlistenFn} from '@tauri-apps/api/event'
import Backdrop from './components/Backdrop.vue'
import BrandMark from './components/BrandMark.vue'
import ConversationBubble from './components/ConversationBubble.vue'
import FeedbackNotices from './components/FeedbackNotices.vue'
import {desktop,call} from './services/api'
import {store,refresh,loadLocal,loadMonitoring,loadTokens,applyUsageRefresh,action} from './stores/store'
import type {UsageRefreshState,ProviderHealth} from './types'
const router=useRouter(),exitOpen=ref(false),cancelScheduled=ref(false),workspace=ref<HTMLElement|null>(null)
const nav=[{path:'/',label:'首页',en:'Home',icon:House},{path:'/tasks',label:'任务',en:'Tasks',icon:CalendarCheck},{path:'/sessions',label:'会话',en:'Sessions',icon:ConversationBubble},{path:'/usage',label:'额度',en:'Usage',icon:ChartPie},{path:'/settings',label:'设置',en:'Settings',icon:Settings}]
const unlisten:UnlistenFn[]=[];let timer:ReturnType<typeof setInterval>
watch(()=>router.currentRoute.value.path,()=>{if(workspace.value){workspace.value.scrollTop=0;workspace.value.scrollLeft=0}},{flush:'post'})
onMounted(async()=>{void refresh();if(desktop){unlisten.push(await listen<string>('notification-open',e=>router.push(e.payload)),await listen('task-updated',()=>loadLocal()),await listen('settings-updated',()=>loadLocal()),await listen('refresh-requested',()=>refresh()),await listen<UsageRefreshState>('usage-updated',e=>{applyUsageRefresh(e.payload);if(!e.payload.refreshing){void loadMonitoring().catch(()=>undefined);void loadTokens()}}),await listen<ProviderHealth>('provider-updated',e=>{const i=store.providers.findIndex(p=>p.id===e.payload.id);if(i<0)store.providers.push(e.payload);else store.providers[i]=e.payload}),await listen<string>('navigate',e=>router.push('/'+e.payload)),await listen('exit-requested',()=>{exitOpen.value=true}));const target=await call<string|null>('get_launch_target');if(target)await router.push(target);timer=setInterval(async()=>{store.now=Date.now();if(!document.hidden)await loadLocal()},15000)}})

let windowStateVersion=0,windowStateClosed=false,saveSizeTimer:ReturnType<typeof setTimeout>
async function syncWindowShape(){
 const version=++windowStateVersion
 try{const window=getCurrentWindow(),[maximized,fullscreen]=await Promise.all([window.isMaximized(),window.isFullscreen()]);if(!windowStateClosed&&version===windowStateVersion)document.documentElement.dataset.windowState=maximized||fullscreen?'maximized':'normal'}catch{/* Keep normal rounded shape when no native window is available. */}
}
onMounted(async()=>{if(!desktop)return;await syncWindowShape();const stop=await getCurrentWindow().onResized(()=>{void syncWindowShape();clearTimeout(saveSizeTimer);saveSizeTimer=setTimeout(()=>{if(!windowStateClosed)void call('save_window_layout').catch(()=>undefined)},600)});if(windowStateClosed)stop();else unlisten.push(stop)})
onUnmounted(()=>{windowStateClosed=true;windowStateVersion++;clearTimeout(saveSizeTimer);unlisten.forEach(f=>f());clearInterval(timer)})
async function windowAction(type:'minimize'|'maximize'|'close'){if(!desktop)return;if(type==='close'){await getCurrentWindow().close();return}await action(()=>type==='minimize'?getCurrentWindow().minimize():getCurrentWindow().toggleMaximize(),'')}
</script>
<template><FeedbackNotices/><div class="desktop-shell" :data-page="$route.path"><Backdrop/><header class="titlebar" data-tauri-drag-region @dblclick="windowAction('maximize')"><RouterLink to="/" class="brand"><BrandMark/><span>Code<span class="brand-accent">Beat</span></span></RouterLink><div class="titlebar-drag" data-tauri-drag-region/><div class="window-controls"><button :title="tr('最小化')" :aria-label="tr('最小化窗口')" @click="windowAction('minimize')"><Minus :size="20"/></button><button :title="tr('最大化 / 还原')" :aria-label="tr('最大化或还原窗口')" @click="windowAction('maximize')"><Square :size="17"/></button><button class="window-close" :title="tr('关闭')" :aria-label="tr('关闭窗口')" @click="windowAction('close')"><X :size="22"/></button></div></header>
<div class="app-shell"><aside class="sidebar"><nav :aria-label="tr('主导航')"><RouterLink v-for="item in nav" :key="item.path" :to="item.path" :class="{active:$route.path===item.path}"><component :is="item.icon"/><span>{{store.settings?.language==='en-US'?item.en:item.label}}</span></RouterLink></nav><div class="sidebar-bottom"><strong>Code<span>Beat</span></strong><p><span class="sidebar-tagline-line">{{tr("让 AI 编程的时间，")}}</span><span class="sidebar-tagline-line">{{tr("更有节奏")}}</span></p><span v-if="!desktop" class="connection-note">{{tr("浏览器预览 · 未连接桌面服务")}}</span></div></aside><div ref="workspace" class="workspace"><main><RouterView/></main></div></div></div>
<div v-if="exitOpen" class="modal-backdrop"><section class="modal small exit-confirm" role="dialog" aria-modal="true" aria-labelledby="exit-title"><h2 id="exit-title">{{tr("退出 CodeBeat")}}</h2><p>{{tr("已注册的系统任务可以在主程序退出后独立执行。")}}</p><label class="exit-option"><input v-model="cancelScheduled" type="checkbox"><span>{{tr("同时取消所有尚未执行的任务")}}</span></label><div class="modal-actions"><button @click="exitOpen=false">{{tr("返回")}}</button><button class="primary" @click="action(()=>call('exit_app',{cancelScheduled}))">{{tr("退出应用")}}</button></div></section></div></template>

<style scoped>
.exit-confirm>.exit-option{display:flex;align-items:center;gap:9px;line-height:22px}
.exit-option>input{width:18px;height:18px;flex:0 0 18px;margin:0;accent-color:var(--primary)}
.exit-option>span{min-width:0}
</style>
