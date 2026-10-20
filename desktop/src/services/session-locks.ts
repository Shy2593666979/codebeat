import {api} from './api'
import type {Conversation} from '../types'

// 只探测本地文件锁，不重新读取会话内容，不启动模型请求。
export async function refreshSessionLocks(conversations:Conversation[]){
 const codex=conversations.filter(c=>c.provider==='codex')
 for(let start=0;start<codex.length;start+=1000){
  const batch=codex.slice(start,start+1000)
  try{
   const states=await api.sessionLocks(batch)
   for(const c of batch)c.lock_state=states[`${c.provider}:${c.session_id}`]||'unknown'
  }catch{
   for(const c of batch)c.lock_state='unknown'
  }
 }
}
