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
  const base=runtime?.day_start?shiftMinute(runtime.day_start,date,settings.timezone):actual[0]?.a??plan.starts[0]!
  const latest=actual.at(-1)
  const resets=[...new Set(actual.filter(w=>w.closed_at).map(w=>w.b))].filter(t=>t>base&&t<end).sort((a,b)=>a-b)
  // 已发生的窗口保留真实起点；不能把延迟发送或后来使用塞回理论五小时格子。
  const confirmed:AllocationNode[]=actual.map(w=>({minute:w.a,executeMinute:null,confirmed:true,provider,source:w.source??'observed',events:[w]}))
  let pending:AllocationNode[]
  if(runtime?.slots){
   pending=runtime.slots.filter(slot=>!latest||Date.parse(slot.execute_at)>Date.parse(latest.start)).map((slot,index)=>({
    minute:Math.max(shiftMinute(slot.display_at,date,settings.timezone),latest?latest.b+index*300:-Infinity),
    executeMinute:Math.max(shiftMinute(slot.execute_at,date,settings.timezone),latest?latest.b+1+index*301:-Infinity),
    confirmed:false,provider,source:'planned',events:[],
   }))
  }else{
   const start=latest?.b??base
   const times=latest?Array.from({length:Math.max(0,Math.ceil((end-start)/300))},(_,index)=>start+index*300)
    :runtime?.display_times?.length?runtime.display_times.map(t=>shiftMinute(t,date,settings.timezone))
    :Array.from({length:Math.max(0,Math.ceil((end-base)/300))},(_,index)=>base+index*300)
   pending=times.map((minute,index)=>({minute,executeMinute:Math.max(base+(confirmed.length+index)*301,latest?latest.b+1+index*301:minute+index),confirmed:false,provider,source:'planned',events:[]}))
  }
  pending=pending.filter(n=>n.minute<end&&n.executeMinute!==null&&n.executeMinute<end&&(!resets.length||end-n.minute>=(settings.min_work_minutes??60)))
  const nodes=[...confirmed,...pending].sort((a,b)=>a.minute-b.minute)
  const history=actual.map((w,index)=>({...w,visualEnd:Math.min(w.b,actual[index+1]?.a??w.b)}))
  const cycles=confirmed.map((node,index)=>({...node,a:node.minute,b:history[index]!.visualEnd}))
  return {provider,nodes,actual:history,resets,cycles,starts:pending.map(n=>n.minute),predictions:pending.map(n=>({display:n.minute,a:n.minute,b:n.minute+300}))}
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
