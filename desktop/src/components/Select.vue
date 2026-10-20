<script setup lang="ts" generic="T extends string | number">
import {tr} from '../composables/i18n'
import {computed,nextTick,onBeforeUnmount,onMounted,ref,useId,watch} from 'vue'
import {Check,ChevronDown,Search} from 'lucide-vue-next'

export interface SelectOption<V extends string | number = string | number> {
  value: V
  label: string
  disabled?: boolean
}
const props=withDefaults(defineProps<{
  modelValue:T|null
  options:readonly SelectOption<T>[]
  id?:string
  placeholder?:string
  disabled?:boolean
  searchable?:boolean
  size?:'small'|'medium'
  label?:string
  required?:boolean
  ariaLabel?:string
}>(),{placeholder:'请选择',searchable:true,size:'medium'})
const emit=defineEmits<{'update:modelValue':[value:T];change:[value:T]}>()
const uid=useId(),open=ref(false),search=ref(''),active=ref(-1)
const root=ref<HTMLElement>(),trigger=ref<HTMLButtonElement>(),panel=ref<HTMLElement>(),searchInput=ref<HTMLInputElement>()
const nativePopover=typeof HTMLElement!=='undefined'&&'showPopover' in HTMLElement.prototype
const panelStyle=ref<Record<string,string>>({})
const triggerId=computed(()=>props.id||uid+'-trigger'),listId=uid+'-list'
const selected=computed(()=>props.options.find(o=>o.value===props.modelValue))
const filtered=computed(()=>props.options.filter(o=>o.label.toLocaleLowerCase().includes(search.value.trim().toLocaleLowerCase())))
const accessibleName=computed(()=>tr(props.ariaLabel||props.label||props.placeholder))
const optionId=(index:number)=>uid+'-option-'+index
const activeId=computed(()=>open.value&&active.value>=0?optionId(active.value):undefined)
let typeahead='',typeTimer:ReturnType<typeof setTimeout>|undefined,observer:ResizeObserver|undefined

function lastEnabled(){return filtered.value.map((o,i)=>o.disabled?-1:i).filter(i=>i>=0).at(-1)??-1}
function resetActive(){const index=filtered.value.findIndex(o=>o.value===props.modelValue&&!o.disabled);active.value=index>=0?index:filtered.value.findIndex(o=>!o.disabled)}
function position(){
  if(!open.value||!trigger.value)return
  const box=trigger.value.getBoundingClientRect(),gap=6,margin=10
  if(box.bottom<=0||box.top>=innerHeight||box.right<=0||box.left>=innerWidth){close();return}
  const below=innerHeight-box.bottom-margin-gap,above=box.top-margin-gap
  const up=below<230&&above>below,available=Math.max(0,up?above:below)
  const width=Math.min(Math.max(box.width,220),innerWidth-margin*2)
  panelStyle.value={left:Math.max(margin,Math.min(box.left,innerWidth-width-margin))+'px',width:width+'px',maxHeight:Math.min(320,available)+'px',...(up?{bottom:innerHeight-box.top+gap+'px',top:'auto'}:{top:box.bottom+gap+'px',bottom:'auto'})}
}
function close(restore=false){
  if(!open.value)return
  if(nativePopover&&panel.value?.matches(':popover-open'))panel.value.hidePopover()
  open.value=false;search.value='';active.value=-1
  if(restore)trigger.value?.focus({preventScroll:true})
}
async function show(last=false){
  if(props.disabled||open.value)return
  open.value=true;search.value='';resetActive()
  if(last)active.value=lastEnabled()
  await nextTick()
  if(!open.value)return
  position()
  await nextTick()
  if(!open.value)return
  if(nativePopover)panel.value?.showPopover()
  if(props.searchable)searchInput.value?.focus({preventScroll:true})
  scrollActive()
}
function choose(option:SelectOption<T>){if(option.disabled||props.disabled)return;emit('update:modelValue',option.value);emit('change',option.value);close(true)}
function scrollActive(){void nextTick(()=>panel.value?.querySelector<HTMLElement>('[data-active="true"]')?.scrollIntoView?.({block:'nearest'}))}
function move(direction:number){
  const enabled=filtered.value.map((o,i)=>o.disabled?-1:i).filter(i=>i>=0)
  if(!enabled.length){active.value=-1;return}
  const current=enabled.indexOf(active.value)
  active.value=enabled[(current+direction+enabled.length)%enabled.length]!
  scrollActive()
}
function keydown(event:KeyboardEvent){
  if(props.disabled||event.isComposing)return
  const input=event.target===searchInput.value
  if(event.key==='Escape'&&open.value){event.preventDefault();event.stopPropagation();close(true);return}
  if(event.key==='Tab'){if(open.value)close(input);return}
  if(event.key==='ArrowDown'||event.key==='ArrowUp'){
    event.preventDefault();event.stopPropagation()
    if(!open.value)void show(event.key==='ArrowUp');else move(event.key==='ArrowDown'?1:-1)
    return
  }
  if((event.key==='Home'||event.key==='End')&&open.value&&!input){
    event.preventDefault();active.value=event.key==='Home'?filtered.value.findIndex(o=>!o.disabled):lastEnabled();scrollActive();return
  }
  if(event.key==='Enter'||event.key===' '&&!input){
    event.preventDefault();event.stopPropagation()
    if(!open.value)void show();else{const option=filtered.value[active.value];if(option)choose(option)}
    return
  }
  if(!input&&event.key.length===1&&!event.ctrlKey&&!event.altKey&&!event.metaKey){
    event.preventDefault();typeahead+=event.key.toLocaleLowerCase();clearTimeout(typeTimer);typeTimer=setTimeout(()=>typeahead='',600)
    if(!open.value)void show().then(()=>findPrefix());else findPrefix()
  }
}
function findPrefix(){const index=filtered.value.findIndex(o=>!o.disabled&&o.label.toLocaleLowerCase().startsWith(typeahead));if(index>=0){active.value=index;scrollActive()}}
function outside(event:PointerEvent){if(!root.value?.contains(event.target as Node))close()}
function focusout(event:FocusEvent){if(!root.value?.contains(event.relatedTarget as Node))close()}
function scroll(event:Event){if(!panel.value?.contains(event.target as Node))position()}
watch(search,()=>{resetActive();scrollActive()})
watch(()=>props.options,()=>{resetActive();position()},{deep:true})
watch(()=>props.modelValue,resetActive)
watch(()=>props.disabled,value=>{if(value)close()})
onMounted(()=>{
  document.addEventListener('pointerdown',outside,true);window.addEventListener('resize',position);document.addEventListener('scroll',scroll,true)
  if(typeof ResizeObserver!=='undefined'){observer=new ResizeObserver(position);if(trigger.value)observer.observe(trigger.value)}
})
onBeforeUnmount(()=>{close();clearTimeout(typeTimer);observer?.disconnect();document.removeEventListener('pointerdown',outside,true);window.removeEventListener('resize',position);document.removeEventListener('scroll',scroll,true)})
</script>

<template>
 <div ref="root" class="select-wrapper" :class="[size,{'select-wrapper--open':open,'select-wrapper--disabled':disabled}]" @focusout="focusout">
  <label v-if="label" :for="triggerId" class="select-label">{{label}}</label>
  <button :id="triggerId" ref="trigger" type="button" class="select-control" :disabled="disabled" role="combobox" aria-haspopup="listbox" :aria-label="accessibleName" :aria-expanded="open" :aria-controls="open?listId:undefined" :aria-required="required||undefined" :aria-activedescendant="!searchable?activeId:undefined" @click="open?close():show()" @keydown="keydown">
   <span class="select-value" :class="{'select-placeholder':!selected}">{{selected?.label??tr(placeholder)}}</span><ChevronDown :size="16" class="select-chevron"/>
  </button>
  <input v-if="required" class="select-validity" tabindex="-1" aria-hidden="true" required :disabled="disabled" :value="selected?String(selected.value):''" @invalid.prevent="trigger?.focus()">
  <div v-if="open" ref="panel" class="select-panel" :popover="nativePopover?'manual':undefined" :style="panelStyle" @keydown="keydown">
   <div v-if="searchable" class="select-search"><Search :size="16"/><input ref="searchInput" v-model="search" role="combobox" type="text" :aria-label="tr('搜索')+accessibleName" aria-autocomplete="list" aria-expanded="true" :aria-controls="listId" :aria-activedescendant="activeId" :placeholder="tr('搜索选项…')" autocomplete="off" @keydown.stop="keydown"></div>
   <div :id="listId" class="select-options" role="listbox" :aria-label="accessibleName">
    <div v-for="(option,index) in filtered" :id="optionId(index)" :key="typeof option.value+':'+option.value" role="option" class="select-option" :aria-selected="option.value===modelValue" :aria-disabled="option.disabled||undefined" :data-active="active===index" :class="{'is-selected':option.value===modelValue,'is-disabled':option.disabled}" @pointerdown.prevent @click="choose(option)" @pointermove="!option.disabled&&(active=index)">
     <span class="select-option-label"><slot name="option" :option="option">{{option.label}}</slot></span><Check v-if="option.value===modelValue" :size="15"/>
    </div>
   </div>
   <div v-if="!filtered.length" class="select-empty" role="status">{{search?tr("没有匹配选项"):tr("暂无可选项")}}</div>
  </div>
 </div>
</template>

<style scoped>
.select-wrapper{position:relative;display:inline-flex;flex-direction:column;gap:7px;min-width:0;vertical-align:middle}
.select-label{font-size:13px;font-weight:500;color:var(--text)}
.select-control{display:flex;align-items:center;justify-content:space-between;gap:12px;width:100%;min-width:0;min-height:40px;padding:9px 12px;border:1px solid var(--border);border-radius:9px;background:var(--surface);color:var(--text);font-size:13px;font-weight:400;line-height:1.5;text-align:left;box-shadow:0 1px 2px #234b7304;cursor:pointer}
.select-control:hover:not(:disabled){background:var(--tint);border-color:color-mix(in srgb,var(--primary) 40%,var(--border))}
.select-control:focus-visible,.select-wrapper--open .select-control{outline:none;border-color:var(--primary);box-shadow:0 0 0 3px color-mix(in srgb,var(--primary) 13%,transparent)}
.select-control:disabled{opacity:.5;cursor:not-allowed}.select-value{min-width:0;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.select-placeholder{color:var(--muted)}.select-chevron{flex-shrink:0;color:var(--muted);transition:transform .15s}.select-wrapper--open .select-chevron{transform:rotate(180deg)}
.select-validity{position:absolute;width:1px!important;height:1px!important;min-height:0!important;padding:0!important;opacity:0;pointer-events:none;bottom:0;left:0}
.select-panel{position:fixed;inset:auto;margin:0;z-index:3000;box-sizing:border-box;padding:5px;border:1px solid var(--border);border-radius:12px;background:var(--select-surface,#fff);color:var(--text);box-shadow:0 12px 36px #17345226,0 2px 6px #1734520a;overflow:auto;overscroll-behavior:contain;scroll-padding-top:48px;scroll-padding-bottom:5px;font-family:inherit;line-height:1.5;scrollbar-width:thin}
.select-search{display:flex;align-items:center;gap:8px;padding:5px 9px 9px;border-bottom:1px solid var(--border);margin-bottom:4px;color:var(--muted);position:sticky;top:-5px;background:var(--select-surface,#fff);z-index:1}.select-search>svg{flex-shrink:0}
.select-search .select-search-input,.select-search input{border:0!important;background:transparent!important;box-shadow:none!important;outline:none!important;width:100%;min-width:0;min-height:32px!important;padding:5px 0!important;border-radius:0;font-size:13px;color:var(--text)}
.select-option{display:flex;align-items:center;gap:12px;justify-content:space-between;padding:9px 11px;min-height:37px;border-radius:7px;font-size:13px;cursor:pointer;line-height:1.5}.select-option-label{min-width:0;overflow-wrap:anywhere}.select-option>svg{flex-shrink:0;color:var(--primary)}.select-option.is-selected{color:var(--primary);font-weight:500}.select-option[data-active=true]{background:var(--tint)}.select-option.is-disabled{color:var(--muted);opacity:.5;cursor:not-allowed}.select-empty{padding:18px 12px;text-align:center;font-size:13px;color:var(--muted)}
.small .select-control{min-height:34px;padding:6px 10px;font-size:12px}
:global(:root[data-appearance=dark]) .select-panel{--select-surface:#17263b}
@media(prefers-color-scheme:dark){:global(:root[data-appearance=system]) .select-panel{--select-surface:#17263b}}
@media(prefers-reduced-motion:no-preference){.select-panel{animation:select-in .12s ease-out}@keyframes select-in{from{opacity:0;translate:0 -3px}to{opacity:1;translate:0 0}}}
</style>
