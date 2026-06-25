<template>
  <div class="max-w-8xl mx-auto p-2 space-y-8">
    <div v-for="(thread, threadIndex) in threads" :key="threadIndex" class="space-y-4">
      <h2 class="text-xl font-bold text-gray-800 border-b pb-2 mb-4">
        Sujet : {{ thread.roots[0]?.subject || 'Sans objet' }}
      </h2>
      
      <!-- Affichage de la racine du thread -->
      <MessageNode 
        v-for="(rootMsg, index) in thread.roots" 
        :key="rootMsg.id" 
        :message="rootMsg"
        :depth="0"
        :initially-expanded="threadIndex === 0 && index === 0" 
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { PropType } from 'vue'
import MessageNode from './MessageNode.vue'
// Assurez-vous d'importer vos interfaces ici
import type { ThreadDto } from '../types' 

defineProps({
  threads: {
    type: Array as PropType<ThreadDto[]>,
    required: true
  }
})
</script>