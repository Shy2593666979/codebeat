import {reactive} from 'vue'
import {api} from '../services/api'
import {applySettings,defaultSettings,store} from './store'
import type {Settings} from '../types'
export const settingsSaveState=reactive({saving:false,pending:false,error:''})
let pending:Partial<Settings>={},timer:ReturnType<typeof setTimeout>|undefined,flight:Promise<void>|undefined
const clone=<T>(value:T):T=>JSON.parse(JSON.stringify(value))
/** 全局唯一的写入队列，保证编辑和页面切换过程中的写入顺序。 */
export function queueSettingsPatch(patch:Partial<Settings>){
 pending={...pending,...clone(patch)};settingsSaveState.pending=true;settingsSaveState.error=''
 clearTimeout(timer);timer=setTimeout(()=>void flushSettings(),250)
}
export function flushSettings():Promise<void>{
 clearTimeout(timer)
 if(flight)return flight
 if(!Object.keys(pending).length)return Promise.resolve()
 settingsSaveState.saving=true
 flight=(async()=>{
  while(Object.keys(pending).length){
   const sent=pending;pending={}
   try{const saved=await api.saveSettings({...clone(store.settings||defaultSettings),...sent});store.settings=saved;applySettings(saved);settingsSaveState.error=''}
   catch(e){pending={...sent,...pending};settingsSaveState.error='未能应用更改：'+String(e).replace(/^Error: /,'');break}
  }
 })().finally(()=>{settingsSaveState.saving=false;settingsSaveState.pending=Object.keys(pending).length>0;flight=undefined})
 return flight
}
