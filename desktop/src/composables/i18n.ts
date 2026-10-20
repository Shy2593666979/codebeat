import {computed,ref} from 'vue'
import zh from '../locales/zh-CN.json'
import en from '../locales/en-US.json'
import messages from '../locales/ui-en.json'
export const language=ref<'zh-CN'|'en-US'>('zh-CN')
export const t=computed(()=>language.value==='en-US'?en:zh)
const english:Readonly<Record<string,string>>=messages
/** 只翻译系统自带的文案；用户文本和原始日志保持原样。 */
export function tr(source:string|null|undefined,parameters:Record<string,string|number>={}):string{
 if(!source)return ''
 let text=source
 if(language.value==='en-US'){
  const key=source.trim(),translated=english[key]
  if(translated!==undefined)text=source.slice(0,source.indexOf(key))+translated+source.slice(source.indexOf(key)+key.length)
  else if(source.startsWith('Error: '))text='Error: '+tr(source.slice(7))
 }
 return text.replace(/\{(\w+)\}/g,(match,key)=>Object.hasOwn(parameters,key)?String(parameters[key]):match)
}
