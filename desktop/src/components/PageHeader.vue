<script setup lang="ts">
import {tr} from '../composables/i18n'
defineProps<{title:string;subtitle:string}>()</script>
<template><div class="page-heading"><div><h1>{{tr(title)}}</h1><p>{{tr(subtitle)}}</p></div><div class="heading-actions"><slot/></div></div></template>
