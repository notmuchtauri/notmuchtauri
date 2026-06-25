use serde::{Deserialize, Serialize};
use tauri_plugin_http::reqwest::Client; // <-- IMPORT MODIFIÉ ICI

// Structures pour formater la requête
#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

// Structures pour parser la réponse
#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Deserialize)]
struct ChatMessageResponse {
    content: String,
}

pub struct LLMWrapper;

impl LLMWrapper {
    pub async fn ask_llm(
        prompt: String,
        context: String,
        api_url: String,
        api_key: String,
        model: String,
    ) -> Result<String, String> {
        // Utilisation du Client de tauri_plugin_http
        let client = Client::new();

        let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: "Tu es un assistant expert pour aider à la rédaction et l'analyse d'e-mails. Réponds de manière concise, professionnelle, et respecte la langue demandée.".to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!("Contexte de l'e-mail original :\n---\n{}\n---\n\nMa demande : {}", context, prompt),
        },
    ];

        let request_body = ChatRequest {
            model,
            messages,
            temperature: 0.7,
        };

        let res = client
            .post(&api_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&&request_body).unwrap());
        let resp = res
            .send()
            .await
            .map_err(|e| format!("Erreur de réseau : {}", e))?;

        if resp.status().is_success() {
            let chat_response: ChatResponse =
                serde_json::from_str(&resp.text().await.unwrap()).unwrap();

            if let Some(choice) = chat_response.choices.first() {
                Ok(choice.message.content.clone())
            } else {
                Err("L'API n'a renvoyé aucune réponse (choices vide).".to_string())
            }
        } else {
            let err_text = resp.text().await.unwrap_or_default();
            Err(format!("Erreur de l'API : {}", err_text))
        }
    }
}
