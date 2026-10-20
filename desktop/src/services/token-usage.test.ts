import {describe,it,expect} from 'vitest'
import {tokenDays,tokenLabel,tokenTotal,tokenSeries,tokenCurve} from './token-usage'
import type {TokenReport} from '../types'
describe('daily token charts',()=>{
 it('creates a rolling local calendar including month boundaries and leap days',()=>{
  const now=new Date(2024,2,1,12).getTime()
  expect(tokenDays(null,3,now).map(d=>d.key)).toEqual(['2024-02-28','2024-02-29','2024-03-01'])
 })
 it('does not add cached input a second time and filters providers',()=>{
  const rows=[{date:'2026-10-16',provider:'codex' as const,counts:{input:100,output:20,cache_read:80,cache_write:0},requests:1},{date:'2026-10-16',provider:'claude_code' as const,counts:{input:200,output:30,cache_read:40,cache_write:60},requests:1}]
  expect(tokenTotal(rows)).toEqual({input:300,output:50,total:350})
  expect(tokenTotal(rows,'codex').total).toBe(120)
  const report={days:rows} as TokenReport
  expect(tokenDays(report,1,new Date(2026,9,16,12).getTime())[0].rows).toHaveLength(2)
  const days=tokenDays(report,1,new Date(2026,9,16,12).getTime())
  expect(tokenSeries(days,'all')[0].total).toBe(350)
  expect(tokenSeries(days,'codex')[0].total).toBe(120)
  expect(tokenSeries(days,'claude_code')[0].total).toBe(230)
 })
 it('draws finite smooth curves through zero, peaks and flat days without inventing extra peaks',()=>{
  const curve=tokenCurve([0,100,5,5,80,0],100)
  curve.points.forEach((point,index)=>expect(point.y).toBeCloseTo([140,8,133.4,133.4,34.4,140][index]))
  const commands=[...curve.line.matchAll(/C ([\d.-]+) ([\d.-]+) ([\d.-]+) ([\d.-]+) ([\d.-]+) ([\d.-]+)/g)]
  expect(commands).toHaveLength(5)
  commands.forEach((command,index)=>{
   const y0=curve.points[index].y,y1=Number(command[2]),y2=Number(command[4]),y3=Number(command[6])
   for(let step=0;step<=20;step++){const t=step/20,y=(1-t)**3*y0+3*(1-t)**2*t*y1+3*(1-t)*t*t*y2+t**3*y3;expect(y).toBeGreaterThanOrEqual(Math.min(y0,y3)-.001);expect(y).toBeLessThanOrEqual(Math.max(y0,y3)+.001)}
  })
  expect(tokenCurve([],1).line).toBe('')
  expect(tokenCurve([0],1).line).not.toMatch(/NaN|Infinity/)
 })
 it('formats zero and large values without losing the exact counts used in detail tables',()=>{
  expect(tokenLabel(0)).toBe('0');expect(tokenLabel(1200)).toBe('1.2K');expect(tokenLabel(1234567)).toBe('1.23M')
 })
})
