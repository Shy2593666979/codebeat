import {api} from '../services/api'
import {tr} from '../composables/i18n'
import type {Conversation,ProviderId} from '../types'

export const SESSION_CACHE_TTL=60_000
export interface SessionSnapshot {data:Conversation[];complete:boolean;errors:string[];updatedAt:number}
interface Entry {snapshot?:SessionSnapshot;request?:Promise<SessionSnapshot>}
const entries=new Map<string,Entry>()
const cacheKey=(provider:string,query:string)=>JSON.stringify([provider,query])
export const sessionView={search:'',provider:'all',project:'all',page:1,pageSize:10,selectedId:'',selectedRows:[] as string[]}
export function cachedSessions(provider:string,query:string){return entries.get(cacheKey(provider,query))?.snapshot}
export function sessionCacheFresh(snapshot:SessionSnapshot|undefined){return !!snapshot?.complete&&Date.now()-snapshot.updatedAt<SESSION_CACHE_TTL}
export function readSessions(provider:string,query:string,force=false):Promise<SessionSnapshot>{
 const key=cacheKey(provider,query)
 let entry=entries.get(key)
 if(!entry){entry={};entries.set(key,entry)}
 if(entry.request)return entry.request
 if(!force&&sessionCacheFresh(entry.snapshot))return Promise.resolve(entry.snapshot!)
 const current=entry,ids=(provider==='all'?['codex','claude_code']:[provider]) as ProviderId[]
 current.request=(async()=>{
  const data:Conversation[]=[],seen=new Set<string>(),errors=new Set<string>()
  const results=await Promise.allSettled(ids.map(async id=>{
   let cursor:string|null=null;const cursors=new Set<string>()
   do{
    const result=await api.sessions(id,query,cursor)
    for(const c of result.data){const key=c.provider+':'+c.session_id;if(!seen.has(key)){seen.add(key);data.push(c)}}
    result.warnings.forEach(message=>errors.add(message));cursor=result.next_cursor
    if(cursor){if(cursors.has(cursor))throw Error(tr('会话分页游标重复，请刷新重试。'));cursors.add(cursor)}
   }while(cursor)
  }))
  for(const result of results)if(result.status==='rejected')errors.add(String(result.reason))
  const complete=results.every(result=>result.status==='fulfilled')&&!errors.size
  // 刷新失败时保留上一次的快照，并且不延长它的新鲜度。
  const snapshot:SessionSnapshot=!complete&&current.snapshot?{...current.snapshot,errors:[...errors]}:{data,complete,errors:[...errors],updatedAt:complete?Date.now():0}
  current.snapshot=snapshot
  return snapshot
 })().finally(()=>{current.request=undefined;for(const [old,value] of entries){if(entries.size<=20)break;if(old!==key&&!value.request)entries.delete(old)}})
 return current.request
}
