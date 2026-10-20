import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest'
import {flushPromises,mount} from '@vue/test-utils'
import Select from './Select.vue'

const mounted:ReturnType<typeof mount>[]=[]
function create(props:Record<string,unknown>={}){
 const wrapper=mount(Select,{attachTo:document.body,props:{modelValue:0,options:[{value:0,label:'关闭'},{value:5,label:'5 分钟',disabled:true},{value:10,label:'10 分钟'}],ariaLabel:'提醒时间',...props}})
 mounted.push(wrapper);return wrapper
}
beforeEach(()=>vi.spyOn(HTMLElement.prototype,'getBoundingClientRect').mockReturnValue(new DOMRect(100,100,200,40)))
afterEach(()=>{mounted.forEach(w=>w.unmount());mounted.length=0;vi.restoreAllMocks()})
describe('Select value and interaction contract',()=>{
 it('renders numeric zero and emits the original number without coercion',async()=>{
  const w=create();expect(w.get('[role=combobox]').text()).toContain('关闭')
  await w.get('button').trigger('click');await flushPromises()
  await w.get('[role=option]:last-child').trigger('click')
  expect(w.emitted('update:modelValue')).toEqual([[10]])
 })
 it('keeps string zero distinct from numeric zero and displays an empty-string option',async()=>{
  const w=create({modelValue:'0',options:[{value:0,label:'数字零'},{value:'0',label:'字符串零'},{value:'',label:'全部项目'}]})
  expect(w.get('button').text()).toContain('字符串零')
  await w.setProps({modelValue:''});expect(w.get('button').text()).toContain('全部项目')
  await w.get('button').trigger('click');await flushPromises();await w.get('[role=option]').trigger('click')
  expect(w.emitted('update:modelValue')).toEqual([[0]])
 })
 it('skips disabled options and exposes the active descendant before Enter',async()=>{
  const w=create({searchable:false})
  await w.get('button').trigger('keydown',{key:'ArrowDown'});await flushPromises()
  await w.get('button').trigger('keydown',{key:'ArrowDown'})
  const id=w.get('button').attributes('aria-activedescendant')
  expect(document.getElementById(id!)?.textContent).toContain('10 分钟')
  await w.get('button').trigger('keydown',{key:'Enter'})
  expect(w.emitted('update:modelValue')).toEqual([[10]])
 })
 it('filters results, handles no matches without invalid indices, and restores focus on Escape',async()=>{
  const w=create();await w.get('button').trigger('click');await flushPromises()
  const input=w.get('input[role=combobox]');await input.setValue('不存在')
  await input.trigger('keydown',{key:'ArrowDown'});await input.trigger('keydown',{key:'Enter'})
  expect(w.findAll('[role=option]')).toHaveLength(0);expect(w.emitted('update:modelValue')).toBeUndefined()
  await input.trigger('keydown',{key:'Escape'})
  expect(w.find('.select-panel').exists()).toBe(false);expect(document.activeElement).toBe(w.get('button').element)
 })
 it('does not choose disabled items and closes an open control when disabled dynamically',async()=>{
  const w=create();await w.get('button').trigger('click');await flushPromises()
  await w.get('[aria-disabled=true]').trigger('click');expect(w.emitted('update:modelValue')).toBeUndefined()
  await w.setProps({disabled:true});expect(w.find('.select-panel').exists()).toBe(false)
  expect(w.get('button').attributes('disabled')).toBeDefined()
 })
 it('updates active state when an async option disappears',async()=>{
  const w=create({searchable:false});await w.get('button').trigger('click');await flushPromises()
  await w.setProps({options:[]});await w.get('button').trigger('keydown',{key:'ArrowUp'});await w.get('button').trigger('keydown',{key:'Enter'})
  expect(w.get('.select-empty').text()).toBe('暂无可选项');expect(w.emitted('update:modelValue')).toBeUndefined()
 })
 it('closes on outside pointer events and links a visible label to the trigger',async()=>{
  const w=create({id:'reminder-trigger',label:'提醒时间'});expect(w.get('label').attributes('for')).toBe('reminder-trigger')
  await w.get('button').trigger('click');await flushPromises();document.body.dispatchEvent(new Event('pointerdown',{bubbles:true}));await flushPromises()
  expect(w.find('.select-panel').exists()).toBe(false)
 })
})
