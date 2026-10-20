import type {PretriggerSettings} from '../types'
export const defaultPretrigger:PretriggerSettings={days:'weekdays',work_start:'10:00',work_end:'20:00',lunch_enabled:false,lunch_start:'12:00',lunch_end:'14:00',timezone:Intl.DateTimeFormat().resolvedOptions().timeZone||'Asia/Shanghai',providers:['codex'],mode:'balanced',min_work_minutes:60,auto_trigger:true,wake:true,day_override:null}
const minutes=(s:string)=>{if(!/^([01]\d|2[0-3]):[0-5]\d$/.test(s))throw Error('请输入有效时间');const [h,m]=s.split(':').map(Number);return h!*60+m!}
export function workIntervals(s:PretriggerSettings):[number,number][]{
 const start=minutes(s.work_start);let end=minutes(s.work_end)
 if(start===end)throw Error('上下班时间不能相同')
 if(end<start)end+=1440
 if(!s.lunch_enabled)return [[start,end]]
 let ls=minutes(s.lunch_start),le=minutes(s.lunch_end)
 if(ls<start)ls+=1440
 if(le<=ls)le+=1440
 if(!(start<ls&&ls<le&&le<end))throw Error('午休时间须在工作时间内')
 return [[start,ls],[le,end]]
}
export function validatePretrigger(s:PretriggerSettings){
 if(!s||typeof s!=='object'||!['weekdays','daily'].includes(s.days)||typeof s.lunch_enabled!=='boolean')throw Error('预启动配置无效')
 workIntervals(s)
 if(!['balanced','windows','save'].includes(s.mode??'balanced')||!Number.isInteger(s.min_work_minutes??60)||(s.min_work_minutes??60)<15||(s.min_work_minutes??60)>300)throw Error('优化模式或最短有效时长无效')
 for(const key of ['auto_trigger','wake'] as const)if(s[key]!==undefined&&typeof s[key]!=='boolean')throw Error('预启动配置无效')
 if(s.day_override){const o=s.day_override;if(!/^\d{4}-\d{2}-\d{2}$/.test(o.date)||new Date(o.date+'T00:00:00Z').toISOString().slice(0,10)!==o.date||typeof o.lunch_enabled!=='boolean')throw Error('临时工作日期无效');workIntervals({...s,...o})}
 if(!Array.isArray(s.providers)||!s.providers.length||s.providers.some(p=>!['codex','claude_code'].includes(p))||new Set(s.providers).size!==s.providers.length)throw Error('请选择适用工具')
 new Intl.DateTimeFormat('en',{timeZone:s.timezone}).format()
}
export interface PretriggerPlan {times:string[];max_work_minutes:number;work_minutes_per_window:number[];starts:number[];intervals:[number,number][];effective_windows:number;explanation:string}
export const minuteClock=(value:number)=>{const day=Math.floor(value/1440),t=((value%1440)+1440)%1440;return `${day<0?'前日 ':day>0?'次日 ':''}${String(Math.floor(t/60)).padStart(2,'0')}:${String(t%60).padStart(2,'0')}`}
export function planPretrigger(s:PretriggerSettings):PretriggerPlan{
 validatePretrigger(s)
 const intervals=workIntervals(s),start=intervals[0]![0],end=intervals.at(-1)![1],minimum=s.min_work_minutes??60,mode=s.mode??'balanced'
 let candidates=Array.from({length:60},(_,i)=>start-295+i*5)
 if(!s.lunch_enabled&&end-start>=2*minimum){const n=2+Math.floor((end-start-2*minimum)/300);candidates.push(start+Math.floor((end-start-(n-2)*300)/2)-300)}
 if(mode==='save')candidates=[start]
 let best:{score:number;first:number;starts:number[];work:number[]}|undefined
 for(const first of candidates){
  const starts:number[]=[];for(let t=first;t<end;t+=300)starts.push(t)
  const work=starts.map(t=>intervals.reduce((sum,[a,b])=>sum+Math.max(0,Math.min(b,t+300)-Math.max(a,t)),0))
  const count=work.filter(n=>n>=minimum).length,short=work.filter(n=>n<minimum).reduce((sum,n)=>sum+minimum-n,0)
  const squares=work.reduce((sum,n)=>sum+n*n,0),idle=starts.reduce((sum,t,i)=>sum+Math.min(t+300,end)-Math.max(t,start)-work[i]!,0)
  const score=count*(mode==='windows'?1000000:100000)-short*2000-squares-idle*10-starts.length*500
  if(!best||score>best.score||score===best.score&&first>best.first)best={score,first,starts,work}
 }
 return {times:best!.starts.map(minuteClock),max_work_minutes:Math.max(...best!.work),work_minutes_per_window:best!.work,starts:best!.starts,intervals,effective_windows:best!.work.filter(n=>n>=minimum).length,explanation:mode==='save'?'节省额度模式仅提供建议，不发送额外请求':'按实际工作交集均衡窗口；后续边界为理论预测，执行前须重新验证官方额度'}
}
