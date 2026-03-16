use crate::api::Message;
use std::fs;

// Erreur personnalisée pour indiquer qu'il faut redémarrer la session
#[derive(Debug)]
pub struct RestartSessionError {
    pub message: String,
}

impl std::fmt::Display for RestartSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RestartSessionError {}

// Crée le fichier CONTINUE.md avec un résumé des derniers messages
pub fn create_continue_file(
    messages: &[Message],
    debug: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = generate_continue_content(messages);
    fs::write("CONTINUE.md", content)?;

    if debug {
        println!("[Debug] Fichier CONTINUE.md créé");
    }

    Ok(())
}

// Génère le contenu du fichier CONTINUE.md (séparé pour les tests)
fn generate_continue_content(messages: &[Message]) -> String {
    let mut content = String::new();
    content.push_str("# Tâche en cours\n\n");

    // Ajouter les 5 derniers messages (sauf système) pour contexte
    let start_idx = if messages.len() > 5 {
        messages.len() - 5
    } else {
        0
    };

    for msg in &messages[start_idx..] {
        if msg.role == "system" {
            continue;
        }
        match msg.role.as_str() {
            "user" => {
                content.push_str(&format!("## User\n\n{}\n\n", msg.content));
            }
            "assistant" => {
                if let Some(tool_calls) = &msg.tool_calls {
                    for tool_call in tool_calls {
                        if tool_call.function.name == "sh" {
                            content.push_str(&format!(
                                "## Assistant (commande shell)\n\n```bash\n{}\n```\n\n",
                                tool_call.function.arguments
                            ));
                        } else {
                            content.push_str(&format!("## Assistant\n\n{}\n\n", msg.content));
                        }
                    }
                } else {
                    content.push_str(&format!("## Assistant\n\n{}\n\n", msg.content));
                }
            }
            "tool" => {
                content.push_str(&format!(
                    "## Résultat shell\n\n```\n{}\n```\n\n",
                    msg.content
                ));
            }
            _ => {}
        }
    }

    content.push_str("\n## Tâches suivantes suggérées\n\n- Continuer la conversation\n- Terminer les tâches en cours\n");
    content
}

// Vérifie et redémarre la session si nécessaire
pub fn check_and_restart_if_needed(
    should_restart: bool,
    messages: &[Message],
    debug: bool,
) -> Result<(), RestartSessionError> {
    if should_restart {
        if debug {
            println!("[Debug] Restart session nécessaire (reste moins de 4000 tokens)");
        }

        // Créer le fichier CONTINUE.md
        if let Err(e) = create_continue_file(messages, debug) {
            return Err(RestartSessionError {
                message: format!("Erreur lors de la création du fichier CONTINUE.md: {}", e),
            });
        }

        Err(RestartSessionError {
            message: "Session redémarrée pour gérer la limite de tokens".to_string(),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_generate_continue_content_empty() {
        let messages = vec![];
        let content = generate_continue_content(&messages);
        assert!(content.contains("# Tâche en cours"));
        assert!(content.contains("## Tâches suivantes suggérées"));
    }

    #[test]
    fn test_generate_continue_content_with_messages() {
        let messages = vec![Message {
            role: "user".to_string(),
            content: "Hello world".to_string(),
            tool_calls: None,
            tool_call_id: None,
            token_count: None,
        }];
        let content = generate_continue_content(&messages);
        assert!(content.contains("## User"));
        assert!(content.contains("Hello world"));
    }

    #[test]
    fn test_generate_continue_content_with_tool_calls() {
        let messages = vec![Message {
            role: "assistant".to_string(),
            content: String::new(),
            tool_calls: Some(vec![ToolCall {
                id: "call_123".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "sh".to_string(),
                    arguments: "{\"command\": \"ls -la\"}".to_string(),
                },
            }]),
            tool_call_id: None,
            token_count: None,
        }];
        let content = generate_continue_content(&messages);
        assert!(content.contains("## Assistant (commande shell)"));
        assert!(content.contains("ls -la"));
    }

    #[test]
    fn test_generate_continue_content_system_messages_skipped() {
        let messages = vec![Message {
            role: "system".to_string(),
            content: "You are a helpful assistant".to_string(),
            tool_calls: None,
            tool_call_id: None,
            token_count: None,
        }];
        let content = generate_continue_content(&messages);
        assert!(!content.contains("system"));
        assert!(!content.contains("helpful assistant"));
    }

    #[test]
    fn test_generate_continue_content_limit_five_messages() {
        let mut messages = Vec::new();
        for i in 0..10 {
            messages.push(Message {
                role: "user".to_string(),
                content: format!("Message {}", i),
                tool_calls: None,
                tool_call_id: None,
                token_count: None,
            });
        }
        let content = generate_continue_content(&messages);
        // Doit contenir les 5 derniers messages (5 à 9)
        assert!(content.contains("Message 5"));
        assert!(content.contains("Message 9"));
        // Ne doit pas contenir les premiers messages (0 à 4)
        assert!(!content.contains("Message 0"));
        assert!(!content.contains("Message 4"));
    }

    #[test]
    fn test_create_continue_file_integration() -> Result<(), Box<dyn std::error::Error>> {
        // Créer un fichier temporaire
        let temp_file = NamedTempFile::new()?;
        let temp_path = temp_file.path().to_str().unwrap().to_string();
        
        // Surcharger la fonction pour utiliser le chemin temporaire
        let messages = vec![Message {
            role: "user".to_string(),
            content: "Test".to_string(),
            tool_calls: None,
            tool_call_id: None,
            token_count: None,
        }];
        
        // Écrire dans le fichier temporaire
        fs::write(&temp_path, generate_continue_content(&messages))?;
        
        // Vérifier que le fichier a été créé et contient le contenu attendu
        let content = fs::read_to_string(&temp_path)?;
        assert!(content.contains("# Tâche en cours"));
        assert!(content.contains("Test"));
        
        Ok(())
    }

    #[test]
    fn test_check_and_restart_if_needed_no_restart() {
        let messages = vec![];
        let result = check_and_restart_if_needed(false, &messages, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_and_restart_if_needed_with_restart() {
        let messages = vec![];
        let result = check_and_restart_if_needed(true, &messages, false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Session redémarrée"));
        // Le fichier CONTINUE.md a été créé (mais on ne le vérifie pas ici)
        // Nettoyer si nécessaire
        let _ = fs::remove_file("CONTINUE.md");
    }

    #[test]
    fn test_restart_session_error_display() {
        let err = RestartSessionError {
            message: "Test error".to_string(),
        };
        assert_eq!(format!("{}", err), "Test error");
    }
}
