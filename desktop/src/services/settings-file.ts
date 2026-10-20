import {validatePretrigger} from './pretrigger'
import type {Settings} from '../types'
import {defaultSettings} from '../stores/store'
export function parseSettingsFile(text:string):Settings {
 const input=JSON.parse(text);if(!input||typeof input!=='object'||Array.isArray(input))throw new Error('配置文件格式无效')
 if(input.application&&input.application!=='codebeat')throw new Error('这不是 CodeBeat 配置文件')
 if(input.version!==undefined&&input.version!==1)throw new Error('不支持此配置文件版本')
 const values=input.settings??input;if(!values||typeof values!=='object'||Array.isArray(values))throw new Error('配置内容无效')
 const result=JSON.parse(JSON.stringify(defaultSettings)) as Settings
 for(const key of Object.keys(defaultSettings) as (keyof Settings)[]){if(!Object.hasOwn(values,key))continue;const expected=defaultSettings[key],value=values[key];if(key==='pretrigger'){validatePretrigger(value);result.pretrigger={...structuredClone(defaultSettings.pretrigger),...structuredClone(value)};continue;}if(Array.isArray(expected)){if(!Array.isArray(value)||value.length>100||value.some(v=>typeof v!=='string'))throw new Error('配置数组格式无效：'+key)}else if(typeof value!==typeof expected)throw new Error('配置字段类型无效：'+key);Object.assign(result,{[key]:value})}
 if(result.connection_interval_minutes===15)result.connection_interval_minutes=10
 if(!['zh-CN','en-US'].includes(result.language)||!['indigo','teal','navy','warm'].includes(result.theme)||!['system','light','dark'].includes(result.appearance))throw new Error('外观或语言配置无效')
 if(!/^([01]\d|2[0-3]):[0-5]\d$/.test(result.default_time)||!Number.isInteger(result.reminder_minutes)||result.reminder_minutes<0||result.reminder_minutes>60||![5,10,30,60].includes(result.connection_interval_minutes))throw new Error('时间配置无效')
 validatePretrigger(result.pretrigger)
 if(result.paused_providers.some(p=>!['codex','claude_code'].includes(p)))throw new Error('提供商配置无效')
 return result
}
