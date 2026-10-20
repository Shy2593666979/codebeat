<script setup lang="ts">
import {computed} from 'vue'
import {LoaderCircle} from 'lucide-vue-next'
import {store} from '../stores/store'
import {resetState,useResetCredit} from '../services/reset-credits'
import {tr} from '../composables/i18n'
import ResetCreditIcon from './ResetCreditIcon.vue'
withDefaults(defineProps<{layout?:'tile'|'inline'}>(),{layout:'tile'})
const credits=computed(()=>store.usage.codex?.reset_credits)
const disabled=computed(()=>resetState.busy||store.busy||store.usageRefresh.codex?.retrying||!credits.value?.account_key||!credits.value.available_count)
</script>
<template>
 <div class="reset-credits" :class="layout" data-testid="reset-credits">
  <ResetCreditIcon/>
  <div class="reset-credit-content"><div class="reset-credit-copy"><span class="credit-label">{{tr('重置卡')}}</span><strong class="credit-count">{{credits?tr('{count} 张',{count:credits.available_count}):tr('暂无法获取')}}</strong></div>
   <button type="button" :disabled="disabled" :aria-busy="resetState.busy" :title="tr('使用 1 张重置卡，重置官方允许的额度窗口')" @click="useResetCredit"><LoaderCircle v-if="resetState.busy" class="spin" :size="12"/>{{resetState.busy?tr('处理中…'):tr('使用重置卡')}}</button>
  </div>
  <p v-if="resetState.message||resetState.error" :role="resetState.error?'alert':'status'" class="reset-credit-feedback" :class="{error:resetState.error}">{{resetState.error||resetState.message}}</p>
 </div>
</template>
<style scoped>
.reset-credits{display:grid;grid-template-columns:34px minmax(0,1fr);column-gap:9px;row-gap:5px;min-width:0;align-items:start}
.reset-credit-content{min-width:0}
.reset-credit-copy{min-width:0}
.reset-credits .credit-label{display:block;font-size:11px;color:var(--muted,#7182a3);line-height:1.4}
.reset-credits .credit-count{display:block;font-size:15px;line-height:1.5;margin:2px 0 4px;font-weight:600;color:var(--text,#17264b);overflow-wrap:anywhere}
.reset-credits button{font-size:11px;min-height:25px;padding:3px 8px;gap:4px;border:1px solid color-mix(in srgb,var(--primary) 16%,var(--border));border-radius:6px;color:var(--primary);background:color-mix(in srgb,var(--primary) 6%,var(--surface));white-space:nowrap}
.reset-credits button>svg{width:12px;height:12px;margin:0}
.reset-credits button:hover:not(:disabled){background:color-mix(in srgb,var(--primary) 12%,var(--surface))}
.reset-credits button:disabled{opacity:.48;cursor:default}
.reset-credits.inline{align-items:center;column-gap:6px}
.inline button{min-height:28px;padding:4px 10px}
.inline .reset-credit-content{display:flex;align-items:center;justify-content:space-between;gap:7px;flex-wrap:wrap}
.inline .reset-credit-copy{display:flex;align-items:baseline;gap:3px;flex-wrap:wrap}
.inline .credit-label{white-space:nowrap}
.inline .credit-count{font-size:12px;font-weight:600;margin:0;white-space:nowrap}
.reset-credit-feedback{grid-column:1/-1;font-size:10px;line-height:1.5;margin:0;color:var(--primary);overflow-wrap:anywhere}
.reset-credit-feedback.error{color:#cb4b46}
</style>
