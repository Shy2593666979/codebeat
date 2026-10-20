import {describe,it,expect} from 'vitest'
import {mount} from '@vue/test-utils'
import {canSchedule,usageText,localInput,remainingPercent,windowLabel} from './api'
import {store} from '../stores/store'
import UsageCard from '../components/UsageCard.vue'
import type {Conversation} from '../types'
describe('Production usage rendering',()=>{
 it('never fabricates usage when no provider data exists',()=>{store.providers=[];store.usage={};const wrapper=mount(UsageCard,{props:{provider:'codex'}});expect(wrapper.text()).toContain('未知');expect(wrapper.text()).not.toContain('8/40');expect(wrapper.find('[role=progressbar]').exists()).toBe(false)})
 it('renders remaining weekly quota without a source or observation timestamp',()=>{store.usage.codex={provider:'codex',status:'available',message:'',windows:[{label:'weekly',used_percent:42,resets_at_utc:'2026-10-20T00:00:00Z',duration_minutes:10080,observed_at_utc:new Date().toISOString(),source:'official RPC fixture',reliability:'official'}]};const w=mount(UsageCard,{props:{provider:'codex'}});expect(w.text()).toContain('58%');expect(w.text()).toContain('一周窗口');expect(w.text()).toContain('剩余');expect(w.text()).not.toContain('10080');expect(w.find('.source-note').exists()).toBe(false);expect(w.text()).not.toContain('official RPC fixture');expect(w.find('[role=progressbar]').attributes('aria-valuenow')).toBe('58');expect(w.find('.progress span').attributes('style')).toContain('width: 58%')})
 it('shows Claude unavailable honestly',()=>{const w=mount(UsageCard,{props:{provider:'claude_code'}});expect(w.text()).toContain('未知');expect(w.find('.usage-link').exists()).toBe(false)})
})
describe('Task boundaries',()=>{
 it('blocks missing and unresumable sessions',()=>{expect(canSchedule(null)).toBe(false);expect(canSchedule({resumable:false} as Conversation)).toBe(false);expect(canSchedule({resumable:true} as Conversation)).toBe(true)})
 it('formats only valid percentages',()=>{for(const v of [null,NaN,-1,101])expect(usageText(v)).toBe('未知');expect(usageText(0)).toBe('0%')})
 it('creates datetime-local without losing the local date',()=>{const d=new Date(2026,9,16,23,30);expect(localInput(d)).toBe('2026-10-16T23:30')})
})

describe('Readable quota windows and remaining amounts',()=>{
 it('shows five hours and one week using server durations',()=>{expect(windowLabel(300,'primary')).toBe('五小时窗口');expect(windowLabel(10080,'secondary')).toBe('一周窗口');expect(windowLabel(1440,'custom')).toBe('1 天窗口');expect(windowLabel(90,'custom')).toBe('1 小时 30 分钟窗口');expect(windowLabel(null,'custom')).toBe('custom');expect(windowLabel(300,'primary','en-US')).toBe('5-hour window')})
 it('inverts valid official usage and preserves unknown values',()=>{expect(remainingPercent(25)).toBe(75);expect(remainingPercent(0)).toBe(100);expect(remainingPercent(100)).toBe(0);for(const value of [null,NaN,-1,101])expect(remainingPercent(value)).toBeNull()})
})
