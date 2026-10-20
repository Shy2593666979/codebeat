<script setup lang="ts">
import {tr} from '../composables/i18n'
import {nextTick,onBeforeUnmount,onMounted,ref} from 'vue'
import {FileText,Pencil,Play,Pause,Trash2} from 'lucide-vue-next'
const props=defineProps<{x:number;y:number;state:string;busy?:boolean}>()
const emit=defineEmits<{close:[];action:[action:'details'|'edit'|'run'|'pause'|'delete']}>()
const panel=ref<HTMLElement>(),left=ref(props.x),top=ref(props.y)
const opener=document.activeElement as HTMLElement|null
const buttons=()=>Array.from(panel.value?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')||[])
function restore(){if(opener?.isConnected)opener.focus({preventScroll:true})}
function close(){restore();emit('close')}
function choose(action:'details'|'edit'|'run'|'pause'|'delete'){restore();emit('action',action)}
function outside(event:PointerEvent){if(!panel.value?.contains(event.target as Node))close()}
function scroll(event:Event){if(!panel.value?.contains(event.target as Node))close()}
function keydown(event:KeyboardEvent){
 const available=buttons(),index=available.indexOf(document.activeElement as HTMLButtonElement)
 if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){
  event.preventDefault()
  const next=event.key==='Home'?0:event.key==='End'?available.length-1:(index+(event.key==='ArrowDown'?1:-1)+available.length)%available.length
  available[next]?.focus()
 }else if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close()}
 else if(event.key==='Tab')close()
}
onMounted(async()=>{
 await nextTick()
 if(!panel.value)return
 const rect=panel.value.getBoundingClientRect()
 left.value=Math.max(8,Math.min(props.x,innerWidth-rect.width-8));top.value=Math.max(8,Math.min(props.y,innerHeight-rect.height-8))
 buttons()[0]?.focus({preventScroll:true})
 document.addEventListener('pointerdown',outside,true);window.addEventListener('scroll',scroll,true);window.addEventListener('resize',close)
})
onBeforeUnmount(()=>{document.removeEventListener('pointerdown',outside,true);window.removeEventListener('scroll',scroll,true);window.removeEventListener('resize',close)})
</script>
<template><Teleport to="body"><div ref="panel" role="menu" :aria-label="tr('任务操作')" class="task-context-menu" :style="{left:left+'px',top:top+'px'}" @keydown="keydown" @contextmenu.prevent>
 <button role="menuitem" @click="choose('details')"><FileText/>{{tr('查看详情')}}</button>
 <button role="menuitem" :disabled="busy" @click="choose('edit')"><Pencil/>{{tr('编辑任务')}}</button>
 <button role="menuitem" :disabled="busy||state!=='scheduled'" @click="choose('run')"><Play/>{{tr('立即运行')}}</button>
 <button role="menuitem" :disabled="busy||!['scheduled','paused'].includes(state)" @click="choose('pause')"><Pause/>{{state==='paused'?tr('恢复任务'):tr('暂停任务')}}</button>
 <div class="menu-divider" role="separator"/>
 <button role="menuitem" class="delete-action" :disabled="busy" @click="choose('delete')"><Trash2/>{{tr('删除任务')}}</button>
</div></Teleport></template>
<style scoped>
.task-context-menu{position:fixed;z-index:105;min-width:194px;max-width:calc(100vw - 16px);padding:6px;border:1px solid var(--border);border-radius:12px;background:#fff;box-shadow:0 12px 38px #152e5830}
.task-context-menu button{display:flex;justify-content:flex-start;width:100%;gap:10px;padding:10px 12px;border:0;background:transparent;font-size:14px;min-height:40px}
.task-context-menu button:hover:not(:disabled),.task-context-menu button:focus-visible{background:var(--tint);color:var(--primary)}
.task-context-menu button svg{width:18px;height:18px}
:global(html[data-appearance=dark]) .task-context-menu{background:#17263b}
@media(prefers-color-scheme:dark){:global(html[data-appearance=system]) .task-context-menu{background:#17263b}}
.task-context-menu .delete-action{color:var(--danger)}
.menu-divider{height:1px;background:var(--border);margin:4px 6px}
</style>
