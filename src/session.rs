use crate::api::Message;

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

    std::fs::write("CONTINUE.md", content)?;

    if debug {
        println!("[Debug] Fichier CONTINUE.md créé");
    }

    Ok(())
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
