import {describe,it,expect} from 'vitest'
import {parseSettingsFile} from './settings-file'
import {defaultSettings} from '../stores/store'
describe('Settings import boundary',()=>{
 it('round trips a versioned export without changing safe defaults',()=>{const parsed=parseSettingsFile(JSON.stringify({application:'codebeat',version:1,settings:{...defaultSettings,default_time:'08:30',notification_sound:false}}));expect(parsed.default_time).toBe('08:30');expect(parsed.notification_sound).toBe(false);expect(parsed.pretrigger_enabled).toBe(true)})
 it('rejects invalid types and unavailable execution settings',()=>{for(const settings of [{notifications:'true'},{default_time:'25:00'},{authorized_roots:[1]},{pretrigger:{days:"broken"}},{connection_interval_minutes:1},{paused_providers:['unknown']}])expect(()=>parseSettingsFile(JSON.stringify({settings}))).toThrow()})
 it('rejects foreign and future formats',()=>{expect(()=>parseSettingsFile('{"application":"other"}')).toThrow();expect(()=>parseSettingsFile('{"version":999}')).toThrow();expect(()=>parseSettingsFile('[]')).toThrow()})
})

it('accepts the ten-minute interval and normalizes legacy fifteen-minute imports',()=>{
 for(const interval of [5,10,30,60])expect(parseSettingsFile(JSON.stringify({connection_interval_minutes:interval})).connection_interval_minutes).toBe(interval)
 expect(parseSettingsFile(JSON.stringify({connection_interval_minutes:15})).connection_interval_minutes).toBe(10)
})
