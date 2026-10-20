<script setup lang="ts">
import ScheduleEditor from './ScheduleEditor.vue'
import type {Conversation,Task} from '../types'
defineProps<{task?:Task;conversation?:Conversation;immediate?:boolean}>()
defineEmits<{close:[];saved:[]}>()
</script>
<template><ScheduleEditor :task="task" :conversation="conversation" :immediate="immediate" modal @close="$emit('close')" @saved="$emit('saved')"/></template>
