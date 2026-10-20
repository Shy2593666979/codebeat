<script setup lang="ts">
import {tr} from '../composables/i18n'

withDefaults(defineProps<{modelValue:boolean;label:string;disabled?:boolean}>(),{disabled:false})
defineEmits<{'update:modelValue':[value:boolean]}>()
</script>
<template><button type="button" role="switch" :aria-label="tr(label)" :aria-checked="modelValue" :disabled="disabled" class="toggle-switch" :class="{on:modelValue}" @click="$emit('update:modelValue',!modelValue)"><span/></button></template>

<style scoped>
/* 悬停时保持轨道稳定；只有实际的状态变化才移动滑块。 */
.toggle-switch,.toggle-switch:hover:not(:disabled){background:#9eb6d4;border:0;transition:none}
.toggle-switch.on,.toggle-switch.on:hover:not(:disabled){background:linear-gradient(100deg,#0078ff,#0794ff)}
.toggle-switch:active:not(:disabled){transform:none}
.toggle-switch span{pointer-events:none}
@media(prefers-reduced-motion:reduce){.toggle-switch span{transition:none}}
</style>
