import type {CreateTaskRequest} from '../types'
import {language,tr} from '../composables/i18n'
export function relativeTime(value:string|null,now=Date.now()):string{
 if(!value)return tr('未知')
 const seconds=Math.ceil((new Date(value).getTime()-now)/1000)
 if(!Number.isFinite(seconds))return tr('未知')
 if(seconds<=0)return tr('等待检查')
 const minutes=Math.ceil(seconds/60),hours=Math.floor(minutes/60),days=Math.floor(hours/24)
 if(days)return tr('{days} 天 {hours} 小时后',{days,hours:hours%24})
 return hours?tr('{hours} 小时 {minutes} 分钟后',{hours,minutes:minutes%60}):tr('{minutes} 分钟后',{minutes})
}
export function scheduleLabel(request:CreateTaskRequest){
 const time=request.scheduled_local.split('T')[1]||''
 switch(request.recurrence?.kind){
  case 'daily':return tr('每天 {time}',{time})
  case 'weekly':{const index=new Date(request.scheduled_at_utc).getDay(),day=(language.value==='en-US'?['Sun','Mon','Tue','Wed','Thu','Fri','Sat']:['日','一','二','三','四','五','六'])[index];return tr('每周{day} {time}',{day,time})}
  case 'every_hours':return tr('每 {hours} 小时',{hours:request.recurrence.hours})
  default:return tr('单次 {time}',{time})
 }
}
export function durationText(milliseconds:number){const minutes=Math.floor(Math.max(0,milliseconds)/60000);return tr('{hours} 小时 {minutes} 分钟',{hours:Math.floor(minutes/60),minutes:minutes%60})}
