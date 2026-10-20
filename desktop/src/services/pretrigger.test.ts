import {describe,it,expect} from 'vitest'
import {defaultPretrigger,planPretrigger,validatePretrigger} from './pretrigger'
import {parseSettingsFile} from './settings-file'
describe('AWO work schedule quota planning',()=>{
 it('starts with automatic triggering enabled and lunch disabled',()=>{expect(defaultPretrigger.lunch_enabled).toBe(false);expect(defaultPretrigger.auto_trigger).toBe(true);expect(planPretrigger(defaultPretrigger).work_minutes_per_window).toEqual([150,300,150])})
 it('retains the original lunch example',()=>{expect(planPretrigger({...defaultPretrigger,lunch_enabled:true})).toMatchObject({times:['07:00','12:00','17:00'],work_minutes_per_window:[120,180,180]})})
 for(const lunchEnd of ['13:00','13:30','14:00'])it(`excludes lunch until ${lunchEnd} without stopping the clock`,()=>{const p=planPretrigger({...defaultPretrigger,work_start:'09:00',work_end:'18:00',lunch_enabled:true,lunch_end:lunchEnd});expect(p.work_minutes_per_window.reduce((a,b)=>a+b,0)).toBe(540-(Number(lunchEnd.slice(0,2))*60+Number(lunchEnd.slice(3))-720));expect(p.starts.every((t,i)=>i===0||t-p.starts[i-1]===300)).toBe(true)})
 it('supports overnight shifts and lunch after midnight',()=>{const p=planPretrigger({...defaultPretrigger,work_start:'22:00',work_end:'08:00',lunch_enabled:true,lunch_start:'02:00',lunch_end:'03:00'});expect(p.intervals).toEqual([[1320,1560],[1620,1920]]);expect(p.times.some(t=>t.startsWith('次日'))).toBe(true)})
 it('ignores stored lunch times while disabled',()=>{expect(()=>validatePretrigger({...defaultPretrigger,lunch_start:'',lunch_end:''})).not.toThrow()})
 it('rejects empty, invalid or reversed lunch and equal shifts',()=>{for(const change of [{work_start:''},{work_end:'10:00'},{lunch_enabled:true,lunch_end:'11:00'},{work_start:'25:00'},{providers:[]},{days:'all'},{timezone:'bogus'},{min_work_minutes:0},{mode:'bad'}])expect(()=>validatePretrigger({...defaultPretrigger,...change} as typeof defaultPretrigger)).toThrow()})
 it('save mode never recommends an extra early request',()=>{expect(planPretrigger({...defaultPretrigger,mode:'save'}).times[0]).toBe('10:00')})
 it('fills missing allocation fields with the enabled defaults',()=>{const old=parseSettingsFile(JSON.stringify({pretrigger:{days:'daily',work_start:'09:00',work_end:'18:00',lunch_enabled:false,lunch_start:'',lunch_end:'',timezone:'Asia/Shanghai',providers:['codex']}}));expect(old.pretrigger.auto_trigger).toBe(true);expect(old.pretrigger.mode).toBe('balanced')})
})
