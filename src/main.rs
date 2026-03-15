// Déclaration des modules
mod agent;
mod api;
mod config;
mod security;
mod shell;
mod token_management;

// Imports
use tokio;

use crate::agent::{Agent, RestartSessionError};
use crate::config::Config;
use crate::token_management::default_max_context_tokens_for_model;

// ------------------ Main ------------------
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Charger la configuration depuis l'environnement
    let config = Config::from_env()?;

    // Calculer la limite de tokens par défaut si non spécifiée
    let max_context_tokens = config
        .max_context_tokens
        .or_else(|| Some(default_max_context_tokens_for_model(&config.model)));

    // Boucle de redémarrage de session
    let mut restart_count = 0;
    const MAX_RESTARTS: u32 = 10;

    loop {
        // Créer l'agent
        let mut agent = Agent::new(
            config.api_key.clone(),
            config.model.clone(),
            config.system_prompt.clone(),
            config.whitelist.clone(),
            config.blacklist.clone(),
            config.max_history_messages,
            max_context_tokens,
            config.debug,
            config.max_retries,
            config.retry_delay_ms,
            config.max_retry_delay_ms,
            config.shell_timeout_ms,
            config.stream,
        );

        // Exécuter l'agent
        match agent.run().await {
            Ok(()) => {
                // Session terminée normalement (utilisateur a tapé 'quit')
                break;
            }
            Err(e) => {
                // Vérifier si c'est une erreur de redémarrage de session
                if let Some(_restart_err) = e.downcast_ref::<RestartSessionError>() {
                    restart_count += 1;
                    if restart_count > MAX_RESTARTS {
                        eprintln!("Trop de redémarrages de session ({}). Arrêt.", MAX_RESTARTS);
                        break;
                    }
                    println!(
                        "🔁 Redémarrage de la session ({}). Création du fichier CONTINUE.md...",
                        restart_count
                    );
                    // Le fichier CONTINUE.md a déjà été créé par l'agent
                    // On continue la boucle pour créer un nouvel agent
                } else {
                    // Autre erreur, on propage
                    return Err(e);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_management::*;

    #[test]
    fn test_default_max_context_tokens_for_model() {
        // Test avec modèle non spécifié (défaut deepseek-chat)
        assert_eq!(default_max_context_tokens_for_model(&None), 112_000);

        // Test deepseek-chat
        assert_eq!(
            default_max_context_tokens_for_model(&Some("deepseek-chat".to_string())),
            112_000
        );

        // Test deepseek-reasoner
        assert_eq!(
            default_max_context_tokens_for_model(&Some("deepseek-reasoner".to_string())),
            104_000
        );

        // Test modèle inconnu
        assert_eq!(
            default_max_context_tokens_for_model(&Some("deepseek-coder".to_string())),
            28_000
        );
        assert_eq!(
            default_max_context_tokens_for_model(&Some("gpt-4".to_string())),
            28_000
        );
    }

    #[test]
    fn test_estimate_tokens() {
        // Test avec texte vide
        assert_eq!(estimate_tokens(""), 0);

        // Test avec texte court (moins de 3 caractères)
        assert_eq!(estimate_tokens("hi"), 1);

        // Test avec texte plus long
        let text = "Hello world, this is a test."; // 28 caractères
        let expected = 28 / 3; // 9
        assert_eq!(estimate_tokens(text), expected);
    }

    #[test]
    fn test_parse_streaming_chunk() {
        use crate::api::{ChatChunk, ChunkChoice, ChunkDelta};
        use serde_json;

        let json = r#"{
            "choices": [
                {
                    "delta": {
                        "content": "Hello"
                    },
                    "index": 0
                }
            ]
        }"#;
        let chunk: ChatChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.choices.len(), 1);
        assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_parse_streaming_chunk_with_tool_calls() {
        use crate::api::{ChatChunk, ChunkChoice, ChunkDelta, FunctionCallDelta, ToolCallDelta};
        use serde_json;

        // Exemple hypothétique d'un chunk avec tool_call delta
        let json = r#"{
            "choices": [
                {
                    "delta": {
                        "tool_calls": [
                            {
                                "index": 0,
                                "id": "call_123",
                                "type": "function",
                                "function": {
                                    "name": "sh",
                                    "arguments": "{\"command\": \"ls\"}"
                                }
                            }
                        ]
                    },
                    "index": 0
                }
            ]
        }"#;
        let chunk: ChatChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.choices.len(), 1);
        let tool_calls = chunk.choices[0].delta.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].index, Some(0));
        assert_eq!(tool_calls[0].id.as_ref(), Some(&"call_123".to_string()));
        assert_eq!(
            tool_calls[0].call_type.as_ref(),
            Some(&"function".to_string())
        );
        let function = tool_calls[0].function.as_ref().unwrap();
        assert_eq!(function.name.as_ref(), Some(&"sh".to_string()));
        assert_eq!(
            function.arguments.as_ref(),
            Some(&"{\"command\": \"ls\"}".to_string())
        );
    }
}
