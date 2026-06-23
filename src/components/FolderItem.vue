<script setup lang="ts">
import { computed, ref } from 'vue';
import type { FolderNode } from '../types';

const props = defineProps<{
  node: FolderNode;
  selectedPath: string | null;
}>();

const emit = defineEmits(['select'])

// --- Logique d'Expansion ---
// Rétracté par défaut
const isExpanded = ref(false)

// Vérifie si le dossier a des sous-dossiers
const hasChildren = computed(() => {
  return props.node.children && props.node.children.length > 0
})

const isSelected = computed(() => {
  return props.selectedPath === props.node.path
})

// Ouvre ou ferme le dossier au clic sur la flèche
const toggleExpand = () => {
  if (hasChildren.value) {
    isExpanded.value = !isExpanded.value
  }
}

// Ouvre le dossier ET le sélectionne au clic sur le texte
const onSelect = () => {
  emit('select', props.node.path)
  
  // Optionnel : Dé-commentez ces lignes si vous voulez que cliquer sur un dossier
  // l'ouvre automatiquement (en plus de charger ses messages).
   if (hasChildren.value && !isExpanded.value) {
     isExpanded.value = true
   }
}
</script>

<template>
  <div>
    <!-- Ligne principale du dossier -->
    <div 
      @click="onSelect"
      class="flex items-center py-1.5 pr-2 pl-1 mx-1 rounded-md cursor-pointer transition-colors group select-none text-sm"
      :class="isSelected ? 'bg-blue-100 text-blue-800 dark:bg-blue-900/40 dark:text-blue-300 font-medium' : 'text-gray-700 dark:text-zinc-300 hover:bg-gray-100 dark:hover:bg-zinc-800'"
    >
      <!-- Bouton Chevron (Clic = Expand/Collapse uniquement) -->
      <!-- Le @click.stop évite de sélectionner le dossier quand on veut juste l'ouvrir -->
      <div 
        class="w-5 h-5 flex items-center justify-center rounded hover:bg-gray-200 dark:hover:bg-zinc-700 transition-colors mr-1"
        @click.stop="toggleExpand"
        :class="hasChildren ? 'cursor-pointer' : 'cursor-default opacity-0'"
      >
        <svg 
          class="w-3.5 h-3.5 text-gray-400 transform transition-transform duration-200"
          :class="isExpanded ? 'rotate-90' : ''"
          fill="none" stroke="currentColor" viewBox="0 0 24 24"
        >
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
        </svg>
      </div>

      <!-- Icône de dossier (Optionnelle, pour le look) -->
      <svg class="w-4 h-4 mr-2 opacity-60" :class="isSelected ? 'text-blue-600 dark:text-blue-400' : ''" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
      </svg>

      <!-- Nom du dossier -->
      <!-- Attention: Adaptez 'node.name' avec la propriété exacte de votre objet FolderNode -->
      <span class="truncate flex-1">{{ node.name }}</span>
    </div>

    <!-- SOUS-DOSSIERS (Récursivité) -->
    <!-- Affiché seulement si ouvert ET possède des enfants -->
    <div v-show="isExpanded" v-if="hasChildren" class="pl-4 mt-0.5">
      <FolderItem
        v-for="child in node.children"
        :key="child.path"
        :node="child"
        :selected-path="selectedPath"
        @select="$emit('select', $event)" 
      />
      <!-- Note: @select="$emit('select', $event)" permet de faire "remonter" l'événement 
           des petits-enfants jusqu'au FolderTree principal -->
    </div>
  </div>
</template>
