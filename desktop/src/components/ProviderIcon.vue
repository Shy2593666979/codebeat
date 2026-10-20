<script setup lang="ts">
import {CodeXml} from 'lucide-vue-next'
import openaiMark from '../assets/openai.svg?no-inline'
import claudeMark from '../assets/claude.svg?no-inline'
import type {ProviderId} from '../types'
withDefaults(defineProps<{provider?:ProviderId;code?:boolean;size?:'small'|'normal'|'large'}>(),{size:'normal'})
</script>
<template><span class="provider-mark" :class="[provider||'generic',size,{'code-mark':code}]" aria-hidden="true"><CodeXml v-if="code||!provider"/><img v-else :class="provider==='codex'?'openai-knot':'claude-symbol'" :src="provider==='codex'?openaiMark:claudeMark" alt=""/></span></template>
<style scoped>
.provider-mark>img{display:block;width:76%;height:76%;max-width:100%;max-height:100%;object-fit:contain;object-position:center;min-width:0;min-height:0}
</style>
