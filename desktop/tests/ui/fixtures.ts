import {defaultPretrigger,planPretrigger} from '../../src/services/pretrigger'
import type {Page} from '@playwright/test'
import type {Settings,Conversation,Task,UsageReport,Monitoring} from '../../src/types'
// Strictly test-only: injected before the production bundle loads. No fixture enters src/ or the installer.
export async function installDesktopFixture(page:Page){
 await page.exposeFunction("__testPlanPretrigger",planPretrigger)
 const now=new Date('2026-10-16T06:40:00Z').getTime()
 await page.clock.install({time:now})
 const settings:Settings={language:'zh-CN',theme:'indigo',close_to_tray:true,autostart:true,paused:false,notifications:true,reminder_minutes:10,pretrigger_enabled:true,pretrigger:structuredClone(defaultPretrigger),authorized_roots:['/web/app','/server/api','D:\\Projects'],save_output:false,startup_minimized:true,default_time:'06:00',default_wake:true,default_catch_up:true,notify_start:true,notify_success:true,notify_failure:true,notification_sound:true,connection_interval_minutes:30,appearance:'light',paused_providers:[]}
 const names=['实现用户认证系统','重构前端组件结构','修复 API 接口问题','优化数据库查询性能','Landing page refactor','Build pipeline cleanup','添加数据可视化图表','修复移动端兼容性问题']
 const roots=['/web/app','/web/app','/server/api','/server/db','/web/landing','/infra/ci','/web/dashboard','/web/mobile']
 const sessions:Conversation[]=names.map((title,index)=>({provider:index%2===0?'codex':'claude_code',session_id:'a27dfaa2-8399-401d-9428-'+String(index).padStart(12,'0'),title,cwd:roots[index],last_updated_utc:new Date(now-index*3*3600000).toISOString(),resumable:true,source:'test fixture metadata'}))
 const taskNames=['Codex 自动继续任务','Claude Code 自动继续任务','每日回归测试','凌晨构建任务','接口巡检任务','文档整理任务']
 const prompts=['监控额度并在指定时间继续项目开发','监控额度并在指定时间继续原有会话','运行单元测试并生成测试报告','构建项目并运行基础检查','调用关键接口并生成健康报告','整理会话内容并更新项目文档']
 const states=['scheduled','scheduled','paused','needs_review','scheduled','succeeded']
 const minutes=[80,135,930,440,225,1210]
 const tasks:Task[]=taskNames.map((title,index)=>{const time=new Date(now+minutes[index]*60000);return {id:'b27dfaa2-8399-401d-9428-'+String(index).padStart(12,'0'),request:{conversation:sessions[index],title,prompt:prompts[index],scheduled_at_utc:time.toISOString(),scheduled_local:new Date(time.getTime()+8*3600000).toISOString().slice(0,16),timezone_id:'Asia/Shanghai',wake_to_run:true,catch_up_policy:'run_on_next_start',catch_up_ttl_minutes:720,permission_profile:'read_only',consent:true,notify:true,recurrence:index===4?{kind:'every_hours',hours:6}:{kind:'daily'}},state:states[index],message:null,created_at:new Date(now-86400000).toISOString(),updated_at:new Date(now).toISOString()}})
 const usage=(provider:'codex'|'claude_code',used:number,observed=now):UsageReport=>({provider,status:'available',message:'已读取官方结构化数据',windows:[{label:'五小时窗口',used_percent:used,duration_minutes:300,resets_at_utc:new Date(now+(provider==='codex'?80:135)*60000).toISOString(),observed_at_utc:new Date(observed).toISOString(),source:'test-only structured quota fixture',reliability:'official'}]})
 const monitoring:Monitoring={snapshots:[],notifications:[{key:'fixture-notification',at:new Date(now).toISOString()}]}
 for(let i=0;i<7;i++)for(const id of ['codex','claude_code'] as const){const observed=now-(6-i)*86400000;monitoring.snapshots.push({report:usage(id,(id==='codex'?[24,40,62,20,33,45,19]:[55,65,78,49,56,69,31])[i],observed),observed_at:new Date(observed).toISOString()})}
 const tokens={days:Array.from({length:30},(_,i)=>['codex','claude_code'].map(provider=>{const date=new Date(now-(29-i)*86400000).toISOString().slice(0,10);return {date,provider,counts:{input:10000+(i%7)*2000,output:1000+(i%3)*500,cache_read:5000,cache_write:0},requests:3}})).flat(),sources:['codex','claude_code'].map(provider=>({provider,status:'available',files:2,records:30,warnings:[]})),observed_at:new Date(now).toISOString(),timezone:'Asia/Shanghai'}
 await page.addInitScript(({settings,sessions,tasks,monitoring,reports,now,tokens})=>{
  const callbacks=new Map(),events=new Map();let callbackId=0
  const globals=window as any;globals.isTauri=true;globals.__codebeatCalls=[];globals.__inboxNotices=[];globals.__displayScale=0
  globals.__codebeatEmit=(event:string,payload:any)=>{const handler=events.get(event);if(handler)callbacks.get(handler)?.({event,id:handler,payload})}
  globals.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}}
  globals.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:(fn:any)=>{callbacks.set(++callbackId,fn);return callbackId},unregisterCallback:(id:number)=>callbacks.delete(id),invoke:async(command:string,args:any={})=>{
   globals.__codebeatCalls.push({command,args})
   if(command==='plugin:event|listen'){events.set(args.event,args.handler);return events.size}
   if(command==='plugin:window|is_maximized')return !!globals.__windowMaximized
   if(command==='plugin:window|is_fullscreen')return !!globals.__windowFullscreen
   if(command==='get_display_layout')return {selected:globals.__displayScale,effective:globals.__displayScale||1}
   if(command==='set_display_scale'){globals.__displayScale=args.value;return {selected:args.value,effective:args.value||1}}
   if(command==='save_window_layout')return null
   if(command==='can_present_notices')return !document.hidden&&document.hasFocus();
   if(command==='get_pending_notices')return structuredClone(globals.__inboxNotices)
   if(command==='publish_notice'){const notice={...args,task_id:null,page:'tasks',at:new Date(now).toISOString()},index=args.topic?globals.__inboxNotices.findIndex((n:any)=>n.topic===args.topic):-1;if(index>=0)globals.__inboxNotices.splice(index,1,notice);else globals.__inboxNotices.push(notice);globals.__codebeatEmit('notices-changed',structuredClone(globals.__inboxNotices));return null}
   if(command==='acknowledge_notices'){globals.__inboxNotices=globals.__inboxNotices.filter((n:any)=>!args.ids.includes(n.id));globals.__codebeatEmit('notices-changed',structuredClone(globals.__inboxNotices));return null}
   if(command==='ignore_all_notices'){globals.__inboxNotices=[];globals.__codebeatEmit('notices-changed',[]);return null}
   if(command==='open_notice')return null
   if(command==='get_launch_target')return globals.__codebeatLaunchTarget||null
   if(command==='preview_pretrigger')return globals.__testPlanPretrigger(args.settings)
   if(command==='get_pretrigger_runs')return []
   if(command==='get_optimizer_status')return {plans:[],habits:[]}
   if(command==='get_codex_compatibility')return {chatgpt_installed:true,engine_available:false,authenticated:false,execution_verified:false,message:'fixture desktop only'}
   if(command==='get_settings')return structuredClone(settings)
   if(command==='get_tray_snapshot')return structuredClone({settings,tasks,usage:Object.values(reports),inbox:globals.__inboxNotices,layout:globals.__trayLayout||null})
   if(command==='tray_action'){if(args.action==='pause')settings.paused=!settings.paused;return null}
   if(command==='get_default_settings')return {...structuredClone(settings),connection_interval_minutes:10,autostart:false,startup_minimized:false,default_wake:false,default_catch_up:false,notify_start:false}
   if(command==='update_settings'){Object.assign(settings,args.settings);return structuredClone(settings)}
   if(command==='list_tasks')return structuredClone(tasks)
   if(command==='list_run_history')return [{id:'run-1',task_id:tasks[5].id,attempt:1,state:'succeeded',started_at:new Date(now-3600000).toISOString(),finished_at:new Date(now-3500000).toISOString(),exit_code:0,error_code:null,error_message:null,log_path:'fixture-log',pid:1}]
   if(command==='list_providers')return ['codex','claude_code'].map(id=>({id,status:'ready',auth_status:'authenticated',version:id==='codex'?'codex-cli fixture':'Claude Code fixture',executable_path:'D:/fixture/'+id+'.exe',message:'已读取官方登录状态',capabilities:{usage:true,sessions:true,resume:true,unattended:true,structured_events:true}}))
   if(command==='detect_provider')return {id:args.id,status:'ready',auth_status:'authenticated',message:'已连接',version:'fixture',executable_path:'D:/fixture/'+args.id+'.exe',capabilities:{usage:true,sessions:true,resume:true,unattended:true,structured_events:true}}
   if(command==='refresh_usage')return reports[args.id as 'codex'|'claude_code']
   if(command==='get_usage_refresh')return structuredClone(globals.__usageRefresh||Object.values(reports).map((report:any)=>({provider:report.provider,report,refreshing:false,retrying:false,next_retry_at:null,next_check_at:new Date(now+600000).toISOString(),last_error:null})))
   if(command==='get_monitoring')return structuredClone(monitoring)
   if(command==='get_token_usage')return structuredClone(tokens)
   if(command==='get_session_lock_states')return Object.fromEntries(args.conversations.map((c:any)=>[`${c.provider}:${c.session_id}`,globals.__sessionLockStates?.[`${c.provider}:${c.session_id}`]||(c.provider==='codex'?'available':'unsupported')]))
   if(command==='list_conversations')return {data:sessions.filter(c=>c.provider===args.provider&&(!args.search||c.title.toLowerCase().includes(args.search.toLowerCase()))).map(c=>({...c,lock_state:globals.__sessionLockStates?.[`${c.provider}:${c.session_id}`]||(c.provider==='codex'?'available':'unsupported')})),next_cursor:null,warnings:[]}
   if(command==='get_conversation_detail')return {conversation:args.conversation,created_at:new Date(now-4*86400000).toISOString(),message_count:32,file_count:null,truncated:false,messages:[{role:'user',text:'请继续实现用户认证系统。'},{role:'assistant',text:'已完成用户登录与注册接口的核心逻辑实现，包括 JWT 鉴权、密码加盐和用户信息持久化。目前正在调试前端登录表单的集成问题。'}]}
   if(command==='pause_task'){const t=tasks.find(t=>t.id===args.id)!;t.state=args.paused?'paused':'scheduled';return structuredClone(t)}
   if(command==='cancel_task'){tasks.find(t=>t.id===args.id)!.state='cancelled';return null}
   if(command==='create_task'){const task={id:'created-task',request:args.request,state:'scheduled',message:null,created_at:new Date(now).toISOString(),updated_at:new Date(now).toISOString()};tasks.push(task);return task}
   if(command==='update_task'){const task=tasks.find(t=>t.id===args.id)!;task.request=args.request;return structuredClone(task)}
   if(command==='get_run_log')return {text:'{"event":"end","state":"succeeded"}',next_offset:35,total_bytes:35}
   if(command==='get_wake_diagnostics')return {tasks:[],message:'fixture diagnostic'}
   if(command.startsWith('plugin:')||['open_provider','install_maintenance','open_logs_folder','exit_app','run_task_now'].includes(command))return 1
   throw new Error('Unexpected test command: '+command)
  }}
 },{settings,sessions,tasks,monitoring,reports:{codex:usage('codex',20),claude_code:usage('claude_code',40)},now,tokens})
}
