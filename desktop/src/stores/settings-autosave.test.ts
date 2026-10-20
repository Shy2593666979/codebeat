import {beforeEach,afterEach,describe,it,expect,vi} from 'vitest'
vi.mock('../services/api',()=>({desktop:true,api:{saveSettings:vi.fn()}}))
describe('automatic settings persistence',()=>{
 beforeEach(()=>{vi.useFakeTimers();vi.resetModules()})
 afterEach(()=>vi.useRealTimers())
 async function setup(){const {api}=await import('../services/api');const {store,defaultSettings}=await import('./store');store.settings=structuredClone(defaultSettings);const save=vi.mocked(api.saveSettings);save.mockReset().mockImplementation(async value=>JSON.parse(JSON.stringify(value)));const queue=await import('./settings-autosave');return {store,save,...queue}}
 it('coalesces edits and applies appearance without feedback popups',async()=>{
  const {store,save,queueSettingsPatch}=await setup();queueSettingsPatch({notification_sound:false});queueSettingsPatch({appearance:'dark'});
  await vi.advanceTimersByTimeAsync(251);expect(save).toHaveBeenCalledTimes(1);expect(store.settings?.notification_sound).toBe(false);expect(document.documentElement.dataset.appearance).toBe('dark');expect(store.notice).toBe('')
 })
 it('serializes in-flight edits and preserves unrelated changes',async()=>{
  const {store,save,queueSettingsPatch,flushSettings}=await setup();let resolve!:(v:any)=>void;
  save.mockImplementationOnce(value=>new Promise(r=>{resolve=()=>r(value)}));queueSettingsPatch({notification_sound:false});const flight=flushSettings();
  queueSettingsPatch({notification_sound:true});expect(save).toHaveBeenCalledTimes(1);resolve(null);await flight;
  expect(save).toHaveBeenCalledTimes(2);expect(store.settings?.notification_sound).toBe(true)
  store.settings!.paused_providers=['claude_code'];queueSettingsPatch({reminder_minutes:0});await flushSettings();expect(store.settings?.paused_providers).toEqual(['claude_code'])
 })
 it('keeps failures inline and retries the latest desired values',async()=>{
  const {store,save,queueSettingsPatch,flushSettings,settingsSaveState}=await setup();save.mockRejectedValueOnce(Error('offline'));
  queueSettingsPatch({notification_sound:false});await flushSettings();expect(settingsSaveState.error).toContain('offline');expect(store.settings?.notification_sound).toBe(true);expect(store.error).toBe('');
  queueSettingsPatch({appearance:'light'});await flushSettings();expect(store.settings?.notification_sound).toBe(false);expect(settingsSaveState.error).toBe('');expect(settingsSaveState.pending).toBe(false)
 })
 it('flushes immediately when leaving the page',async()=>{
  const {store,save,queueSettingsPatch,flushSettings}=await setup();queueSettingsPatch({default_time:'08:30'});await flushSettings();expect(save).toHaveBeenCalledTimes(1);expect(store.settings?.default_time).toBe('08:30');await vi.advanceTimersByTimeAsync(500);expect(save).toHaveBeenCalledTimes(1)
 })
})
