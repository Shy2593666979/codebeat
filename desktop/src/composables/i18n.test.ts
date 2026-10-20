import {afterEach,describe,expect,it} from 'vitest'
import {language,tr} from './i18n'
import {relativeTime,durationText} from '../services/presentation'
import {stateLabel} from '../types/states'
import messages from '../locales/ui-en.json'
afterEach(()=>language.value='zh-CN')
describe('system interface translations',()=>{
 it('preserves Chinese and replaces named parameters',()=>{expect(tr('近 {days} 天 Token 消耗',{days:7})).toBe('近 7 天 Token 消耗')})
 it('translates live and leaves unknown user content untouched',()=>{language.value='en-US';expect(tr('新建任务')).toBe('New task');expect(tr('  任务  ')).toBe('  Tasks  ');expect(tr('用户自己的项目名称')).toBe('用户自己的项目名称');expect(tr('Error: 请输入有效时间')).toBe('Error: Enter a valid time')})
 it('localizes generated durations and statuses',()=>{language.value='en-US';expect(durationText(80*60000)).toBe('1h 20m');expect(relativeTime(new Date(80*60000).toISOString(),0)).toBe('in 1h 20m');expect(stateLabel('scheduled')).toBe('Scheduled')})
 it('English catalog contains English copy',()=>{expect(Object.values(messages).every(value=>!/[\u3400-\u9fff]/u.test(value))).toBe(true)})
})
