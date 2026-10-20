import {describe,it,expect} from 'vitest'
import {allocationDate,allocationRows,rulerLabels} from './coding-allocation'
import {defaultPretrigger} from './pretrigger'
import type {OptimizerRuntime} from '../types'
const settings={...defaultPretrigger,timezone:'Asia/Shanghai',days:'daily' as const}
describe('daily coding allocation',()=>{
 it('shows regular five-hour boundaries without accumulating execution delays on the ruler',()=>{
  const row=allocationRows(settings,'2026-10-21',Date.parse('2026-10-20T23:00:00Z'),[],[])[0]
  expect(row.nodes.map(n=>n.minute)).toEqual([450,750,1050])
  expect(row.nodes.map(n=>n.executeMinute)).toEqual([450,751,1052])
  expect(row.predictions.map(w=>[w.a,w.b])).toEqual([[450,750],[750,1050],[1050,1350]])
 })
 it('restarts the display cycle at the official reset instead of showing later usage observations as ticks',()=>{
  const runtime={provider:'codex',date:'2026-10-20',day_start:'2026-10-20T00:40:00Z',display_times:['2026-10-20T00:40:00Z','2026-10-20T05:40:00Z','2026-10-20T10:40:00Z'],slots:[]} as unknown as OptimizerRuntime
  const row=allocationRows(settings,'2026-10-20',Date.parse('2026-10-20T10:00:00Z'),[runtime],[
   {provider:'codex',source:'observed',start:'2026-10-20T00:40:00Z',end:'2026-10-20T05:40:00Z'},
   {provider:'codex',source:'observed',start:'2026-10-20T06:01:00Z',end:'2026-10-20T11:01:00Z',closed_at:'2026-10-20T06:43:00Z'},
   {provider:'codex',source:'observed',start:'2026-10-20T07:04:00Z',end:'2026-10-20T12:04:00Z'},
  ])[0]
  expect(row.cycles.map(w=>[w.a,w.b])).toEqual([[520,820],[820,883],[883,1183]])
  expect(row.nodes.map(n=>n.minute)).toEqual([520,820,883])
  expect(row.resets).toEqual([883])
  expect(row.predictions).toEqual([])
  expect(row.cycles[2].events[0].start).toBe('2026-10-20T07:04:00Z')
 })
 it('switches immediately after work, including overnight shifts and weekdays',()=>{
  expect(allocationDate(settings,Date.parse('2026-10-19T11:59:59Z'))).toBe('2026-10-19')
  expect(allocationDate(settings,Date.parse('2026-10-19T12:00:00Z'))).toBe('2026-10-20')
  expect(allocationDate({...settings,days:'weekdays'},Date.parse('2026-10-16T12:00:00Z'))).toBe('2026-10-19')
  const night={...settings,work_start:'22:00',work_end:'08:00'}
  expect(allocationDate(night,Date.parse('2026-10-19T23:59:00Z'))).toBe('2026-10-19')
  expect(allocationDate(night,Date.parse('2026-10-20T00:00:00Z'))).toBe('2026-10-20')
 })
 it('replaces cancelled theoretical slots with confirmed early use and keeps the full history',()=>{
  const runtime={provider:'codex',date:'2026-10-19',actual_reset:'2026-10-19T03:20:00Z',recommendations:['2026-10-19T03:21:00Z','2026-10-19T08:21:00Z'],status:'active'} as OptimizerRuntime
  const windows=[{provider:'codex' as const,start:'2026-10-18T22:20:00Z',end:'2026-10-19T03:20:00Z'}]
  const row=allocationRows(settings,'2026-10-19',Date.parse('2026-10-18T22:30:00Z'),[runtime],windows)[0]
  expect(row.nodes.map(n=>[n.minute,n.confirmed])).toEqual([[380,true],[680,false],[980,false]])
  expect(row.nodes.slice(1).map(n=>n.executeMinute)).toEqual([681,982])
  expect(row.starts).not.toContain(450)
  runtime.actual_reset='2026-10-19T09:00:00Z';runtime.recommendations=['2026-10-19T09:01:00Z']
  windows.push({provider:'codex',start:'2026-10-19T04:00:00Z',end:'2026-10-19T09:00:00Z'})
  expect(allocationRows(settings,'2026-10-19',Date.parse('2026-10-19T04:30:00Z'),[runtime],windows)[0].nodes.map(n=>[n.minute,n.confirmed])).toEqual([[380,true],[680,true],[980,false]])
 })
 it('does not restore cancelled recommendations when an actual window leaves no remaining work',()=>{
  const runtime={provider:'codex',date:'2026-10-19',actual_reset:'2026-10-19T15:00:00Z',window_confirmed:true,recommendations:[],slots:[],status:'active'} as unknown as OptimizerRuntime
  expect(allocationRows(settings,'2026-10-19',Date.parse('2026-10-19T11:00:00Z'),[runtime],[])[0].starts).toEqual([])
 })
 it('keeps close labels on one line by shifting them horizontally',()=>{
  const axis=rulerLabels([430,600,601,602,730,900,1330],430,1330,230)
  expect(axis.labels).toHaveLength(7)
  expect(axis.height).toBe(20)
  expect(axis.labels.every(label=>label.lane===0&&label.style.top==='0px')).toBe(true)
  for(const a of axis.labels)for(const b of axis.labels)if(a.minute<b.minute&&a.lane===b.lane){
   const leftA=parseFloat(a.style.left)*2.3,leftB=parseFloat(b.style.left)*2.3
   expect(leftB-leftA).toBeGreaterThanOrEqual(parseFloat(a.style.width)+3.9)
  }
 })
 it('clips the previous window at an observed official reset',()=>{
  const row=allocationRows(settings,'2026-10-19',Date.parse('2026-10-19T07:10:00Z'),[],[
   {provider:'codex',start:'2026-10-19T06:01:00Z',end:'2026-10-19T11:01:00Z',closed_at:'2026-10-19T06:43:00Z'},
   {provider:'codex',start:'2026-10-19T07:04:00Z',end:'2026-10-19T12:04:00Z'},
  ])[0]
  expect(row.actual[0].b).toBe(883);expect(row.actual[0].visualEnd).toBe(883)
  expect(row.actual[1].a).toBe(904)
 })
 it('does not turn an old active runtime or a moving zero-percent reset into triggered nodes',()=>{
  const runtime={provider:'codex',date:'2026-10-19',actual_reset:'2026-10-19T05:36:00Z',recommendations:['2026-10-19T05:37:00Z'],status:'active'} as unknown as OptimizerRuntime
  const row=allocationRows(settings,'2026-10-19',Date.parse('2026-10-19T00:40:00Z'),[runtime],[])[0]
  expect(row.actual).toEqual([])
  expect(row.nodes.every(node=>!node.confirmed)).toBe(true)
  expect(row.starts).toEqual([450,750,1050])
 })
})
