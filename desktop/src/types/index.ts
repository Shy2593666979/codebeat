export type ProviderId = 'codex' | 'claude_code'
export interface PretriggerSettings {days:'weekdays'|'daily';work_start:string;work_end:string;lunch_enabled:boolean;lunch_start:string;lunch_end:string;timezone:string;providers:ProviderId[];mode?:'balanced'|'windows'|'save';min_work_minutes?:number;auto_trigger?:boolean;wake?:boolean;day_override?:{date:string;work_start:string;work_end:string;lunch_enabled:boolean}|null}
export interface ProbeRecord {provider:ProviderId;slot:string;state:string;message:string;attempted_at:string;finished_at:string|null;next_check_at:string|null;before?:UsageReport|null;after?:UsageReport|null;tokens?:Record<string,number>|null;request_sent?:boolean}
export interface OptimizerRuntime {provider:ProviderId;date:string;work_start:string;work_end:string;intervals:[string,string][];recommendations:string[];actual_reset:string|null;window_confirmed?:boolean;day_start?:string|null;display_times?:string[];slots?:{display_at:string;execute_at:string}[];status:string;reason:string;updated_at:string;automatic_allowed:boolean}
export interface ConfirmedWindow {provider:ProviderId;start:string;end:string;source?:'app'|'app_unconfirmed'|'observed';display_start?:string|null;closed_at?:string|null}
export interface Capabilities { usage:boolean; sessions:boolean; resume:boolean; unattended:boolean; structured_events:boolean }
export interface ProviderHealth {id:ProviderId;executable_path:string|null;version:string|null;status:string;auth_status:string;message:string;capabilities:Capabilities}
export interface UsageWindow {label:string;used_percent:number|null;resets_at_utc:string|null;duration_minutes:number|null;observed_at_utc:string;source:string;reliability:string}
export interface ResetCredits {available_count:number;account_key:string|null;credits?:{id:string;expires_at:string|null;expiry_known:boolean;status:string}[]|null}
export type ResetOutcome='reset'|'alreadyRedeemed'|'nothingToReset'|'noCredit'
export interface UsageReport {provider:ProviderId;windows:UsageWindow[];status:string;message:string;reset_credits?:ResetCredits|null}
export interface UsageRefreshState {provider:ProviderId;report:UsageReport|null;refreshing:boolean;retrying:boolean;next_retry_at:string|null;next_check_at:string;last_error:string|null}
export type SessionLockState='available'|'in_use'|'unknown'|'unsupported'
export interface Conversation {provider:ProviderId;session_id:string;title:string;cwd:string;last_updated_utc:string|null;resumable:boolean;source:string;lock_state?:SessionLockState}
export interface SessionPage {data:Conversation[];next_cursor:string|null;warnings:string[]}
export type Recurrence={kind:'daily'|'weekly'}|{kind:'every_hours';hours:number}
export interface CreateTaskRequest {locked_session_behavior?:'fork'|'pause';recurrence?:Recurrence|null;conversation:Conversation;title:string;prompt:string;scheduled_at_utc:string;scheduled_local:string;timezone_id:string;wake_to_run:boolean;catch_up_policy:'skip'|'ask'|'run_on_next_start';catch_up_ttl_minutes:number;permission_profile:'read_only'|'workspace_write'|'full_access';consent:boolean;notify:boolean}
export interface Task {id:string;request:CreateTaskRequest;state:string;message:string|null;created_at:string;updated_at:string}
export interface RunRecord {id:string;task_id:string;attempt:number;state:string;started_at:string;finished_at:string|null;exit_code:number|null;error_code:string|null;error_message:string|null;log_path:string|null;pid:number|null}
export interface Settings {language:'zh-CN'|'en-US';theme:'indigo'|'teal'|'navy'|'warm';close_to_tray:boolean;autostart:boolean;paused:boolean;notifications:boolean;reminder_minutes:number;pretrigger_enabled:boolean;pretrigger:PretriggerSettings;authorized_roots:string[];save_output:boolean;startup_minimized:boolean;default_time:string;default_wake:boolean;default_catch_up:boolean;notify_start:boolean;notify_success:boolean;notify_failure:boolean;notification_sound:boolean;connection_interval_minutes:number;appearance:'system'|'light'|'dark';paused_providers:ProviderId[]}
export interface Monitoring {snapshots:{report:UsageReport;observed_at:string}[];notifications:{key:string;at:string}[]}
export interface TokenCounts {input:number;output:number;cache_read:number;cache_write:number}
export interface DailyTokens {date:string;provider:ProviderId;counts:TokenCounts;requests:number}
export interface TokenReport {days:DailyTokens[];sources:{provider:ProviderId;status:'available'|'partial'|'empty'|'missing';files:number;records:number;warnings:string[]}[];observed_at:string;timezone:string}
export interface ConversationDetail {conversation:Conversation;created_at:string|null;messages:{role:string;text:string}[];message_count:number|null;file_count:number|null;truncated:boolean}

export interface InboxNotice {topic?:string|null;id:string;title:string;message:string;kind:'success'|'error'|'info';task_id:string|null;page:'tasks'|'usage';at:string}
