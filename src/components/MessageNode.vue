<template>
  <div class="relative w-full">
    
    <!-- Ligne verticale pour montrer la hiérarchie visuelle (visible à partir du niveau 1) -->
    <div 
      v-if="depth > 0" 
      class="absolute left-[-16px] top-6 bottom-0 w-0.5 bg-gray-200"
    ></div>

    <!-- Carte du message -->
    <div 
      class="border rounded-lg mb-3 shadow-sm transition-all duration-200 bg-white overflow-hidden"
      :class="{ 'ml-8': depth > 0 }"
    >
      <!-- Header du message (Cliquable pour Collapse) -->
      <div 
        @click="toggle" 
        class="cursor-pointer p-3 flex justify-between items-center hover:bg-gray-50 transition-colors"
        :class="{ 'bg-gray-100 border-b': isExpanded }"
      >
        <div class="flex items-center space-x-3 overflow-hidden">
          <!-- Avatar minimaliste -->
          <div class="w-8 h-8 rounded-full bg-blue-100 text-blue-600 flex items-center justify-center font-bold flex-shrink-0">
            {{ getInitials(message.from) }}
          </div>
          
          <div class="flex flex-col truncate">
            <span class="font-semibold text-gray-800 text-sm truncate">{{ extractName(message.from) }}</span>
            <span v-if="!isExpanded" class="text-xs text-gray-500 truncate mt-0.5">
               <!-- Aperçu du contenu si le message est replié -->
               {{ snippet }}
            </span>
          </div>
        </div>

        <div class="flex items-center space-x-4 flex-shrink-0 text-sm text-gray-500">
          <span>{{ formattedDate }}</span>
          <!-- Icône Chevron (SVG) -->
          <svg 
            class="w-5 h-5 transform transition-transform" 
            :class="isExpanded ? 'rotate-180' : ''" 
            fill="none" stroke="currentColor" viewBox="0 0 24 24"
          >
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
          </svg>
        </div>
      </div>

      <!-- Corps du message (Affiché si isExpanded est true) -->
      <div v-show="isExpanded" class="p-4 bg-white">
        <!-- En-têtes secondaires -->
        <div class="mb-4 text-sm text-gray-600 space-y-1 bg-gray-50 p-3 rounded">
          <div><span class="font-semibold text-gray-700">De :</span> {{ message.from }}</div>
          <div><span class="font-semibold text-gray-700">À :</span> {{ message.to }}</div>
          <div v-if="message.cc"><span class="font-semibold text-gray-700">Cc :</span> {{ message.cc }}</div>
        </div>

        <!-- Contenu (HTML prioritaire, sinon Texte) -->
        <!--<div v-if="message.htmlBody" class="email-body prose max-w-none text-sm" v-html="message.htmlBody"></div>-->
        <div v-if="processedHtml" class="email-body prose max-w-none text-sm" v-html="processedHtml"></div>

        <pre v-else-if="message.textBody" class="whitespace-pre-wrap font-sans text-sm text-gray-800">{{ message.textBody }}</pre>
        <div v-else class="italic text-gray-400 text-sm">Ce message est vide.</div>

        <!-- Pièces jointes (si présentes) -->
        <div v-if="message.attachments.length > 0" class="mt-6 pt-4 border-t border-gray-100">
          <p class="text-xs font-semibold text-gray-500 mb-2 uppercase tracking-wider">Pièces jointes</p>
          <div class="flex flex-wrap gap-2">
            <div 
              v-for="att in message.attachments" 
              :key="att.partId" 
              class="flex items-center space-x-2 bg-gray-100 px-3 py-2 rounded text-sm hover:bg-gray-200 cursor-pointer"
            >
              <svg class="w-4 h-4 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13" />
              </svg>
              <span class="truncate max-w-xs text-gray-700">{{ att.filename }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Récursion : Affichage des réponses en dessous du message parent -->
    <div v-if="message.replies && message.replies.length > 0" class="w-full">
      <MessageNode 
        v-for="reply in message.replies" 
        :key="reply.id" 
        :message="reply"
        :depth="depth + 1"
        :initially-expanded="false" 
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, PropType, onMounted } from 'vue'
import type { MessageDto } from '../types' // Ajustez le chemin
import { invoke } from '@tauri-apps/api/core'

const props = defineProps({
  message: {
    type: Object as PropType<MessageDto>,
    required: true
  },
  depth: {
    type: Number,
    default: 0
  },
  initiallyExpanded: {
    type: Boolean,
    default: false
  }
})

// État local pour le toggle
const isExpanded = ref(props.initiallyExpanded)

const toggle = () => {
  isExpanded.value = !isExpanded.value
}

// --- NOUVEAU : HTML asynchrone ---
// On crée une ref qui contient le HTML de base, et qu'on va modifier
const processedHtml = ref(props.message.htmlBody || '')

// --- Fonctions utilitaires d'affichage ---
onMounted(async () => {
  // Si le message n'a pas d'images inline ou pas de HTML, on s'arrête là
  if (!props.message.htmlBody || props.message.inlineImages.length === 0) return

  let tempHtml = props.message.htmlBody

  for (const img of props.message.inlineImages) {
    if (!img.contentId) continue

    // Notmuch retourne parfois le content-id avec des chevrons, ex: <mon-image@domaine.com>
    // Dans le HTML, l'attribut src est souvent src="cid:mon-image@domaine.com"
    const rawCid = img.contentId.replace(/^<|>$/g, '')

    try {
      // 1. Demander le binaire encodé en base64 au backend Rust
      const base64Data: string = await invoke('get_message_part', { 
        messageId: props.message.id, 
        partId: img.partId 
      })

      // 2. Construire l'URL Base64
      const mimeType = img.contentType || 'image/png'
      const dataUrl = `data:${mimeType};base64,${base64Data}`

      // 3. Échapper le CID au cas où il contient des caractères spéciaux (points, +...)
      const safeCid = rawCid.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
      
      // 4. Remplacer 'cid:VOTRE_ID' partout dans le HTML (src="", background-image, etc.)
      const regex = new RegExp(`cid:${safeCid}`, 'gi')
      tempHtml = tempHtml.replace(regex, dataUrl)

    } catch (error) {
      console.error(`Impossible de charger l'image inline CID: ${rawCid}`, error)
    }
  }

  // 5. Mettre à jour la vue avec le HTML modifié
  processedHtml.value = tempHtml
})


// Formater la date proprement
const formattedDate = computed(() => {
  if (!props.message.date) return ''
  const d = new Date(props.message.date)
  if (isNaN(d.getTime())) return props.message.date
  return new Intl.DateTimeFormat('fr-FR', {
    day: '2-digit', month: 'short', year: 'numeric',
    hour: '2-digit', minute: '2-digit'
  }).format(d)
})

// Extraire "Prénom Nom" depuis "Prénom Nom <email@domaine.com>"
const extractName = (fromRaw: string) => {
  const match = fromRaw.match(/^([^<]+)/)
  return match ? match[1].replace(/"/g, '').trim() : fromRaw
}

// Générer les initiales (ex: "Olivier Barais" -> "OB")
const getInitials = (fromRaw: string) => {
  const name = extractName(fromRaw)
  const parts = name.split(' ').filter(p => p.length > 0)
  if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase()
  if (parts.length === 1) return parts[0].substring(0, 2).toUpperCase()
  return '?'
}

// Petit aperçu du corps du message quand il est fermé
const snippet = computed(() => {
  const text = props.message.textBody || ''
  const clean = text.replace(/\s+/g, ' ').trim()
  return clean.length > 80 ? clean.substring(0, 80) + '...' : clean || '(Contenu HTML / Sans texte)'
})
</script>

<style scoped>
/* 
  Si vous n'utilisez pas @tailwindcss/typography (plugin "prose"), 
  ajoutez des styles basiques ici pour l'affichage brut de l'email HTML.
*/
:deep(.email-body a) {
  color: #2563eb;
  text-decoration: underline;
}
:deep(.email-body blockquote) {
  border-left: 4px solid #e5e7eb;
  padding-left: 1rem;
  color: #4b5563;
  margin-left: 0;
  margin-right: 0;
}
:deep(.email-body img) {
  max-width: 100%;
  height: auto;
}
</style>