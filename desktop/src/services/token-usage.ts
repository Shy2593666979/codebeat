import type {DailyTokens,ProviderId,TokenReport} from '../types'
export function tokenLabel(value:number){return value>=1e9?(value/1e9).toFixed(2)+'B':value>=1e6?(value/1e6).toFixed(2)+'M':value>=1e3?(value/1e3).toFixed(1)+'K':value.toLocaleString('zh-CN')}
export function tokenDays(report:TokenReport|null,days:number,now:number){
 return Array.from({length:days},(_,index)=>{const date=new Date(now);date.setDate(date.getDate()-days+index+1);const key=[date.getFullYear(),String(date.getMonth()+1).padStart(2,'0'),String(date.getDate()).padStart(2,'0')].join('-');return {key,label:(date.getMonth()+1)+'/'+date.getDate(),rows:report?.days.filter(d=>d.date===key)||[]}})
}
export function tokenTotal(rows:DailyTokens[],provider:ProviderId|'all'='all'){
 return rows.filter(r=>provider==='all'||r.provider===provider).reduce((a,r)=>({input:a.input+r.counts.input,output:a.output+r.counts.output,total:a.total+r.counts.input+r.counts.output}),{input:0,output:0,total:0})
}
export function tokenSeries(days:ReturnType<typeof tokenDays>,provider:ProviderId|'all'){
 return days.map(day=>({...day,total:tokenTotal(day.rows,provider).total}))
}
// 单调三次插值：曲线经过每天的实际计数，又不会在峰值处过冲。
export function tokenCurve(values:number[],scale:number,width=700){
 const baseline=140,top=8
 const points=values.map((value,i)=>({x:(i+.5)*width/values.length,y:baseline-Math.max(0,value)/Math.max(1,scale)*(baseline-top)}))
 if(!points.length)return {points,line:'',area:''}
 const slopes=points.slice(1).map((p,i)=>(p.y-points[i].y)/(p.x-points[i].x))
 const tangents=points.map((_,i)=>i===0?slopes[0]||0:i===points.length-1?slopes.at(-1)||0:slopes[i-1]*slopes[i]<=0?0:2*slopes[i-1]*slopes[i]/(slopes[i-1]+slopes[i]))
 for(let i=0;i<slopes.length;i++){
  if(slopes[i]===0){tangents[i]=0;tangents[i+1]=0;continue}
  const a=tangents[i]/slopes[i],b=tangents[i+1]/slopes[i],length=Math.hypot(a,b)
  if(length>3){tangents[i]*=3/length;tangents[i+1]*=3/length}
 }
 const n=(value:number)=>Number(value.toFixed(3))
 let line=`M ${n(points[0].x)} ${n(points[0].y)}`
 for(let i=1;i<points.length;i++){
  const previous=points[i-1],point=points[i],dx=(point.x-previous.x)/3
  line+=` C ${n(previous.x+dx)} ${n(previous.y+tangents[i-1]*dx)} ${n(point.x-dx)} ${n(point.y-tangents[i]*dx)} ${n(point.x)} ${n(point.y)}`
 }
 return {points,line,area:line+` L ${n(points.at(-1)!.x)} ${baseline} L ${n(points[0].x)} ${baseline} Z`}
}
