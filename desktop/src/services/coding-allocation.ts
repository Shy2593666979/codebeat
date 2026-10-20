import {planPretrigger} from './pretrigger'
import type {PretriggerSettings,OptimizerRuntime,ConfirmedWindow,ProviderId} from '../types'

export function localDay(now:number,timezone:string){return new Intl.DateTimeFormat('sv-SE',{timeZone:timezone,year:'numeric',month:'2-digit',day:'2-digit'}).format(new Date(now))}
const addDay=(date:string,offset:number)=>new Date(Date.parse(date+'T00:00:00Z')+offset*86400000).toISOString().slice(0,10)
export function shiftMinute(iso:string,date:string,timezone:string){
 const instant=new Date(iso),parts=new Intl.DateTimeFormat('en-GB',{timeZone:timezone,hour:'2-digit',minute:'2-digit',hourCycle:'h23'}).formatToParts(instant)
 return Number(parts.find(p=>p.type==='hour')!.value)*60+Number(parts.find(p=>p.type==='minute')!.value)+(Date.parse(localDay(instant.getTime(),timezone))-Date.parse(date))/86400000*1440
}
export function settingsForDay(settings:PretriggerSettings,date:string){return settings.day_override?.date===date?{...settings,...settings.day_override,day_override:null}:settings}
export function allocationDate(settings:PretriggerSettings,now:number){
 const today=localDay(now,settings.timezone),minute=shiftMinute(new Date(now).toISOString(),today,settings.timezone)
 for(let offset=-1;offset<=7;offset++){
  const date=addDay(today,offset)
  if(settings.days==='weekdays'&&[0,6].includes(new Date(date+'T12:00:00Z').getUTCDay()))continue
  const plan=planPretrigger(settingsForDay(settings,date))
  if(offset*1440+plan.intervals.at(-1)![1]>minute)return date
 }
 return addDay(today,1)
}
export interface AllocationNode {minute:number;executeMinute:number|null;confirmed:boolean;provider:ProviderId;source:string;events:ConfirmedWindow[]}
export function allocationRows(settings:PretriggerSettings,date:string,now:number,runtimes:OptimizerRuntime[],windows:ConfirmedWindow[]){
 const plan=planPretrigger(settingsForDay(settings,date)),end=plan.intervals.at(-1)![1]
 return settings.providers.map(provider=>{
  const runtime=runtimes.find(r=>r.provider===provider&&r.date===date)
  const actual=windows.filter(w=>w.provider===provider&&Date.parse(w.start)<=now)
   .map(w=>({...w,a:shiftMinute(w.start,date,settings.timezone),b:shiftMinute(w.closed_at||w.end,date,settings.timezone)}))
   .filter(w=>w.a>=Math.min(0,plan.starts[0]!)&&w.a<end).sort((a,b)=>a.a-b.a)
  const first=actual[0]
  const base=runtime?.day_start?shiftMinute(runtime.day_start,date,settings.timezone):first?shiftMinute(first.display_start||first.start,date,settings.timezone):plan.starts[0]!
  // 官方提前重置会截断旧周期；观测到使用的时间留在详情中，不另加主刻度。
  const resets=[...new Set(actual.filter(w=>w.closed_at).map(w=>w.b))].filter(t=>t>base&&t<end).sort((a,b)=>a-b)
  const anchors=[base,...resets]
  const cycleStart=(minute:number)=>{const anchor=[...anchors].reverse().find(a=>a<=minute)??base;return anchor+Math.max(0,Math.floor((minute-anchor)/300))*300}
  const eventCycle=(w:typeof actual[number])=>cycleStart(Math.max(w.display_start?shiftMinute(w.display_start,date,settings.timezone):w.a,[...resets].reverse().find(r=>r<=w.a)??base))
  const cycleEnd=(minute:number)=>Math.min(minute+300,resets.find(r=>r>minute)??Infinity)
  const times=resets.length?anchors.flatMap((anchor,i)=>Array.from({length:Math.max(0,Math.ceil(((anchors[i+1]??end)-anchor)/300))},(_,n)=>anchor+n*300)):runtime?.display_times?.length?runtime.display_times.map(t=>shiftMinute(t,date,settings.timezone)):Array.from({length:Math.max(0,Math.ceil((end-base)/300))},(_,i)=>base+i*300)
  const nodes:AllocationNode[]=times.map((minute,index)=>{
   const events=actual.filter(w=>eventCycle(w)===minute)
   const slot=runtime?.slots?.find(s=>resets.length?cycleStart(shiftMinute(s.execute_at,date,settings.timezone))===minute:shiftMinute(s.display_at,date,settings.timezone)===minute)
   const latest=actual.at(-1),lastIndex=latest?times.indexOf(cycleStart(latest.a)):-1
   const anchor=[...anchors].reverse().find(a=>a<=minute)??base
   const nominal=resets.length?minute+Math.floor((minute-anchor)/300):base+index*301
   const fallback=latest&&index>lastIndex?Math.max(nominal,latest.b+1+(index-lastIndex-1)*301):nominal
   return {minute,executeMinute:slot?shiftMinute(slot.execute_at,date,settings.timezone):runtime?.slots?null:fallback,confirmed:events.length>0,provider,source:events.some(w=>w.source?.startsWith('app'))?'app':'observed',events}
  }).filter(n=>!resets.length||n.confirmed||(runtime?.slots?n.executeMinute!==null:end-n.minute>=(settings.min_work_minutes??60)))
  const pending=nodes.filter(n=>!n.confirmed&&n.executeMinute!==null)
  return {provider,nodes,actual:actual.map((w,i)=>({...w,visualEnd:Math.min(w.b,actual[i+1]?.a??w.b)})),resets,cycles:nodes.filter(n=>n.confirmed).map(n=>({...n,a:n.minute,b:cycleEnd(n.minute)})),starts:pending.map(n=>n.minute),predictions:pending.map(n=>({display:n.minute,a:n.minute,b:cycleEnd(n.minute)}))}
 })
}

/** 刻度位置保持比例，标签在同一行内左右避让，不换行。 */
export function rulerLabels(values:number[],low:number,high:number,pixels:number){
 const minutes=[...new Set(values)].filter(t=>t>=low&&t<=high).sort((a,b)=>a-b)
 const gap=4,available=Math.max(1,pixels-gap*Math.max(0,minutes.length-1))
 const scale=Math.min(1,available/Math.max(1,minutes.reduce((sum,t)=>sum+(t<0||t>=1440?68:36),0)))
 const boxes=minutes.map(minute=>{const width=(minute<0||minute>=1440?68:36)*scale;return {minute,width,left:Math.max(0,Math.min(pixels-width,(minute-low)/Math.max(1,high-low)*pixels-width/2))}})
 for(let i=1;i<boxes.length;i++)boxes[i]!.left=Math.max(boxes[i]!.left,boxes[i-1]!.left+boxes[i-1]!.width+gap)
 for(let i=boxes.length-1;i>=0;i--){const box=boxes[i]!;box.left=Math.min(box.left,i===boxes.length-1?pixels-box.width:boxes[i+1]!.left-gap-box.width)}
 const labels=boxes.map(({minute,width,left})=>{
  return {minute,position:(minute-low)/Math.max(1,high-low)*100+'%',style:{left:left/Math.max(1,pixels)*100+'%',top:'0px',width:width+'px',fontSize:11*scale+'px',textAlign:(minute===low?'left':minute===high?'right':'center') as 'left'|'right'|'center'},lane:0}
 })
 return {labels,height:20}
}
