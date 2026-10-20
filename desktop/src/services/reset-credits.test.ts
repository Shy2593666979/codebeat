import {beforeEach,afterEach,describe,it,expect,vi} from 'vitest'
import {mount} from '@vue/test-utils'
import {api} from './api'
import {store} from '../stores/store'
import {language} from '../composables/i18n'
import {redemptionAttempt,resetState,useResetCredit} from './reset-credits'
import ResetCredits from '../components/ResetCredits.vue'
import type {UsageReport,ResetOutcome} from '../types'

const report=(count:number|null=2):UsageReport=>({provider:'codex',windows:[],status:'available',message:'',reset_credits:count===null?null:{available_count:count,account_key:'fixture-account'}})
beforeEach(()=>{localStorage.clear();language.value='zh-CN';store.usage.codex=report();store.busy=false;store.resetting=false;Object.assign(resetState,{busy:false,message:'',error:''})})
afterEach(()=>vi.restoreAllMocks())
describe('Reset cards: all redemptions use mocked IPC',()=>{
 it('distinguishes unknown and zero, and never calls consume just by rendering',()=>{
  const consume=vi.spyOn(api,'consumeResetCredit')
  store.usage.codex=report(null)
  const w=mount(ResetCredits)
  expect(w.text()).toContain('暂无法获取');expect(w.get('button').attributes('disabled')).toBeDefined()
  w.unmount();store.usage.codex=report(0)
  const zero=mount(ResetCredits);expect(zero.text()).toContain('0 张');expect(zero.get('button').attributes('disabled')).toBeDefined()
  expect(consume).not.toHaveBeenCalled()
 })
 it('persists the same key across retry/reload and isolates accounts',()=>{
  const first=redemptionAttempt(localStorage,'a')
  expect(redemptionAttempt(localStorage,'a')).toBe(first)
  expect(redemptionAttempt(localStorage,'b')).not.toBe(first)
 })
 it('never sends when durable attempt storage fails',async()=>{
  vi.spyOn(Storage.prototype,'setItem').mockImplementation(()=>{throw Error('storage unavailable')})
  const consume=vi.spyOn(api,'consumeResetCredit')
  await useResetCredit();expect(consume).not.toHaveBeenCalled();expect(resetState.error).toBeTruthy()
 })
 it('blocks repeated clicks and re-reads both limits and count',async()=>{
  let finish!:(outcome:ResetOutcome)=>void
  const consume=vi.spyOn(api,'consumeResetCredit').mockImplementation(()=>new Promise(resolve=>{finish=resolve}))
  const read=vi.spyOn(api,'usage').mockResolvedValue(report(1))
  const first=useResetCredit();await useResetCredit()
  expect(consume).toHaveBeenCalledTimes(1);expect(store.resetting).toBe(true)
  finish('reset');await first
  expect(read).toHaveBeenCalledWith('codex');expect(store.usage.codex?.reset_credits?.available_count).toBe(1)
  expect(localStorage.length).toBe(0);expect(store.resetting).toBe(false)
 })
 it('reuses the same key after a timeout instead of spending another card',async()=>{
  const consume=vi.spyOn(api,'consumeResetCredit').mockRejectedValueOnce(Error('timeout')).mockResolvedValueOnce('alreadyRedeemed')
  vi.spyOn(api,'usage').mockResolvedValue(report(1))
  await useResetCredit();expect(resetState.error).toContain('同一次操作')
  const key=consume.mock.calls[0][0]
  await useResetCredit();expect(consume.mock.calls[1][0]).toBe(key);expect(resetState.message).toContain('已完成')
 })
 it.each<ResetOutcome>(['nothingToReset','noCredit'])('handles %s without reporting a successful reset',async outcome=>{
  vi.spyOn(api,'consumeResetCredit').mockResolvedValue(outcome)
  vi.spyOn(api,'usage').mockResolvedValue(report(0))
  await useResetCredit();expect(resetState.message).toContain('没有');expect(resetState.message).not.toContain('已使用')
 })
 it('does not leave a stale usable count if refresh fails after redemption',async()=>{
  const consume=vi.spyOn(api,'consumeResetCredit').mockResolvedValue('reset')
  vi.spyOn(api,'usage').mockRejectedValue(Error('offline'))
  await useResetCredit();expect(store.usage.codex?.reset_credits).toBeNull();expect(resetState.message).toContain('重置已完成')
  await useResetCredit();expect(consume).toHaveBeenCalledTimes(1)
 })
 it('renders translated labels',()=>{
  language.value='en-US';const w=mount(ResetCredits)
  expect(w.text()).toContain('2 cards');expect(w.get('button').text()).toBe('Use reset card')
 })
})
