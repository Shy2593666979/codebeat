<script setup lang="ts">
import {tr} from '../composables/i18n'

import {nextTick,onBeforeUnmount,onMounted,onUnmounted,ref} from 'vue'
import {X} from 'lucide-vue-next'
const props=withDefaults(defineProps<{title:string;description?:string;wide?:boolean;closeLabel?:string;busy?:boolean;panelClass?:string}>(),{closeLabel:'关闭'})
const emit=defineEmits<{close:[]}>()
const panel=ref<HTMLElement>(),previous=document.activeElement as HTMLElement|null
let backdrop:HTMLElement|undefined
const focusable=()=>Array.from(panel.value?.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),a[href],[tabindex="0"]')||[]).filter(el=>el.getClientRects().length>0)
function close(){if(!props.busy)emit('close')}
function onKey(e:KeyboardEvent){
 const dialogs=document.querySelectorAll('.app-dialog-backdrop')
 if(dialogs[dialogs.length-1]!==backdrop)return
 // 展开的下拉框会先自行处理第一次 Escape 和 Tab，早于对话框。
 if((e.key==='Escape'||e.key==='Tab')&&(e.target as HTMLElement)?.closest('.select-wrapper--open'))return
 if(e.key==='Escape'){e.preventDefault();e.stopPropagation();close()}
 if(e.key==='Tab'){const items=focusable(),first=items[0],last=items.at(-1);if(!first){e.preventDefault();panel.value?.focus()}else if(e.shiftKey&&(document.activeElement===first||document.activeElement===panel.value||!panel.value?.contains(document.activeElement))){e.preventDefault();last?.focus()}else if(!e.shiftKey&&(document.activeElement===last||!panel.value?.contains(document.activeElement))){e.preventDefault();first.focus()}}
}
onMounted(async()=>{backdrop=panel.value?.parentElement||undefined;document.body.classList.add('dialog-open');document.addEventListener('keydown',onKey,true);await nextTick();panel.value?.focus()})
onBeforeUnmount(()=>document.removeEventListener('keydown',onKey,true))
onUnmounted(()=>{if(!document.querySelector('.app-dialog-backdrop'))document.body.classList.remove('dialog-open');if(previous?.isConnected)previous.focus()})
</script>
<template><Teleport to="body"><div class="modal-backdrop app-dialog-backdrop" @mousedown.self="close"><section ref="panel" class="modal app-dialog" :class="[panelClass,{'dialog-wide':wide}]" role="dialog" aria-modal="true" :aria-label="tr(title)" tabindex="-1"><header class="dialog-header"><span v-if="$slots.icon" class="dialog-heading-icon"><slot name="icon"/></span><div><h2>{{tr(title)}}</h2><p v-if="description">{{tr(description)}}</p></div><button type="button" class="icon-button dialog-close" :title="tr(closeLabel)" :aria-label="tr(closeLabel)" :disabled="busy" @click="close"><X :size="20"/></button></header><div class="dialog-body"><slot/></div><footer v-if="$slots.footer" class="dialog-footer"><slot name="footer"/></footer></section></div></Teleport></template>
