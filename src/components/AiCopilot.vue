<template>
  <div 
    class="transition-all duration-300 ease-in-out flex flex-col border border-gray-300 rounded-md bg-gray-50 flex-shrink-0"
    :class="isExpanded ? 'w-80 ml-2' : 'w-10 ml-2 cursor-pointer hover:bg-gray-100'"
  >
    <!-- VUE COLLAPSÉE (Fermée) -->
    <div 
      v-if="!isExpanded" 
      class="flex-1 flex flex-col items-center py-4" 
      @click="isExpanded = true"
      title="Ouvrir l'assistant IA"
    >
      <!-- Icône Étincelles (Sparkles) -->
      <svg class="w-5 h-5 text-purple-600 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 12l-5.714 2.143L13 21l-2.286-6.857L5 12l5.714-2.143L13 3z" />
      </svg>
      <!-- Texte vertical -->
      <span style="writing-mode: vertical-rl" class="text-xs text-gray-500 font-bold tracking-widest uppercase rotate-180">
        Assistant IA
      </span>
    </div>

    <!-- VUE EXPANDÉE (Ouverte) -->
    <div v-else class="flex-1 flex flex-col h-full overflow-hidden">
      <!-- Header de l'IA -->
      <div class="flex justify-between items-center p-3 border-b border-gray-200 bg-white">
        <div class="flex items-center space-x-2 text-purple-600 font-medium text-sm">
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
          </svg>
          <span>Copilote IA</span>
        </div>
        <button @click="isExpanded = false" class="text-gray-400 hover:text-gray-600 focus:outline-none">
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
        </button>
      </div>

      <!-- Zone de Chat -->
      <div class="flex-1 overflow-y-auto p-3 space-y-4 text-sm bg-gray-50" ref="chatContainer">
        <div v-if="chatHistory.length === 0" class="text-center text-gray-400 italic mt-4">
          Comment puis-je vous aider à rédiger ce mail ?
        </div>

        <div v-for="(msg, index) in chatHistory" :key="index" :class="msg.role === 'user' ? 'text-right' : 'text-left'">
          <div 
            :class="[
              'inline-block p-2.5 rounded-lg max-w-[90%] text-left', 
              msg.role === 'user' ? 'bg-purple-600 text-white rounded-br-none' : 'bg-white border border-gray-200 text-gray-800 rounded-bl-none shadow-sm'
            ]"
          >
            {{ msg.content }}
          </div>
          
          <!-- Bouton pour insérer le texte généré par l'IA dans l'e-mail -->
          <div v-if="msg.role === 'assistant'" class="mt-1 flex justify-start">
            <button 
              @click="$emit('insert-text', msg.content)" 
              class="text-xs text-purple-600 hover:text-purple-800 hover:underline flex items-center"
            >
              <svg class="w-3 h-3 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" /></svg>
              Insérer dans l'email
            </button>
          </div>
        </div>

        <div v-if="isTyping" class="text-left">
          <div class="inline-block p-2 bg-white border border-gray-200 rounded-lg rounded-bl-none shadow-sm text-gray-400 text-xs italic flex space-x-1">
            <span class="animate-bounce">.</span><span class="animate-bounce delay-100">.</span><span class="animate-bounce delay-200">.</span>
          </div>
        </div>
      </div>

      <!-- Input de saisie -->
      <div class="p-2 border-t border-gray-200 bg-white">
        <form @submit.prevent="sendMessage" class="flex gap-1 relative">
          <input 
            v-model="userInput" 
            type="text" 
            placeholder="Demandez à l'IA..." 
            class="flex-1 text-sm p-2 border border-gray-300 rounded-md outline-none focus:border-purple-500 focus:ring-1 focus:ring-purple-500 transition-shadow pr-8"
            :disabled="isTyping"
          />
          <button 
            type="submit" 
            :disabled="!userInput.trim() || isTyping"
            class="absolute right-1.5 top-1.5 p-1 text-purple-600 hover:bg-purple-50 rounded disabled:opacity-50"
          >
            <svg class="w-4 h-4 transform rotate-90" fill="currentColor" viewBox="0 0 20 20"><path d="M10.894 2.553a1 1 0 00-1.788 0l-7 14a1 1 0 001.169 1.409l5-1.429A1 1 0 009 15.571V11a1 1 0 112 0v4.571a1 1 0 00.725.962l5 1.428a1 1 0 001.17-1.408l-7-14z" /></svg>
          </button>
        </form>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, nextTick } from 'vue'

const emit = defineEmits(['insert-text'])

const isExpanded = ref(false)
const userInput = ref('')
const isTyping = ref(false)
const chatContainer = ref<HTMLElement | null>(null)

interface ChatMessage {
  role: 'user' | 'assistant'
  content: string
}

const chatHistory = ref<ChatMessage[]>([])

const scrollToBottom = async () => {
  await nextTick()
  if (chatContainer.value) {
    chatContainer.value.scrollTop = chatContainer.value.scrollHeight
  }
}

const sendMessage = async () => {
  if (!userInput.value.trim()) return

  // 1. Ajouter le message utilisateur
  const text = userInput.value
  chatHistory.value.push({ role: 'user', content: text })
  userInput.value = ''
  scrollToBottom()

  // 2. Simuler l'attente de l'IA (Ici vous ferez votre appel Tauri/Backend API)
  isTyping.value = true
  
  // -- REMPLACEZ CECI PAR VOTRE VRAI APPEL IA --
  setTimeout(() => {
    chatHistory.value.push({ 
      role: 'assistant', 
      content: `Voici une suggestion de réponse générée pour : "${text}"` 
    })
    isTyping.value = false
    scrollToBottom()
  }, 1500)
}
</script>