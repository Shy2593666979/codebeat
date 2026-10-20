import {tr} from '../composables/i18n'
const labels:Record<string,string>={pending_registration:'注册中',scheduled:'已排程',registration_failed:'注册失败',paused:'已暂停',claimed:'已抢占',running:'执行中',succeeded:'成功',failed:'失败',blocked_approval:'审批受阻',needs_review:'待人工审查',expired:'已过期',cancelled:'已取消',cancelling:'正在停止',completed_unverified:'完成 · 未验证'}
export function stateLabel(state:string){return tr(labels[state]??state)}
