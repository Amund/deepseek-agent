use crate::api::*;
use crate::interrupt;
use futures::StreamExt;
use reqwest::Response;
use std::io::Write;
use colored::*;



// Pour construire les tool_calls en streaming
#[derive(Debug, Default)]
pub struct ToolCallBuilder {
    index: Option<u32>,
    id: Option<String>,
    call_type: Option<String>,
    function_name: Option<String>,
    function_arguments: String, // Accumule les fragments d'arguments
    converted: bool,
}

impl ToolCallBuilder {
    pub fn is_complete(&self) -> bool {
        !self.converted
            && self.id.is_some()
            && self.call_type.is_some()
            && self.function_name.is_some()
            && !self.function_arguments.is_empty()
    }

    pub fn to_tool_call(&mut self) -> Option<ToolCall> {
        if !self.is_complete() {
            return None;
        }
        // Vérifier que les arguments sont un JSON valide
        if serde_json::from_str::<serde_json::Value>(&self.function_arguments).is_err() {
            return None;
        }
        self.converted = true;
        Some(ToolCall {
            id: self.id.clone().unwrap(),
            call_type: self.call_type.clone().unwrap(),
            function: FunctionCall {
                name: self.function_name.clone().unwrap(),
                arguments: self.function_arguments.clone(),
            },
        })
    }

    pub fn update_from_delta(&mut self, delta: &ToolCallDelta) {
        if self.converted {
            return;
        }
        if let Some(index) = delta.index {
            self.index = Some(index);
        }
        if let Some(id) = &delta.id {
            self.id = Some(id.clone());
        }
        if let Some(call_type) = &delta.call_type {
            self.call_type = Some(call_type.clone());
        }
        if let Some(function) = &delta.function {
            if let Some(name) = &function.name {
                self.function_name = Some(name.clone());
            }
            if let Some(arguments) = &function.arguments {
                self.function_arguments.push_str(arguments);
            }
        }
    }
}

pub struct StreamProcessor {
    pub debug: bool,
}

impl StreamProcessor {
    pub fn new(debug: bool) -> Self {
        Self { debug }
    }

    // Traite une réponse streaming et retourne un ChatResponse
    pub async fn process_response(
        &self,
        resp: Response,
    ) -> Result<ChatResponse, Box<dyn std::error::Error>> {
        let mut accumulated_message = Message {
            role: "assistant".to_string(),
            content: String::new(),
            tool_calls: None,
            tool_call_id: None,
            token_count: None,
        };
        let mut tool_call_builders: Vec<ToolCallBuilder> = Vec::new();
        let mut accumulated_tool_calls: Vec<ToolCall> = Vec::new();
        let mut usage: Option<Usage> = None;



        // Lire le stream ligne par ligne
        let stream = resp.bytes_stream();
        tokio::pin!(stream);
        let mut buffer = String::new();
        let mut stream_done = false;
        let mut interrupted = false;
        let mut prefix_printed = false;

        while let Some(item) = stream.next().await {
            if stream_done {
                break;
            }
            match item {
                Ok(chunk_bytes) => {
                    let chunk_str = String::from_utf8_lossy(&chunk_bytes);
                    if self.debug {
                        println!("[Debug] Raw chunk: {:?}", chunk_str);
                    }
                    buffer.push_str(&chunk_str);

                    // Traiter les lignes complètes dans le buffer
                    let all_lines: Vec<String> =
                        buffer.split('\n').map(|s| s.to_string()).collect();
                    let mut lines = all_lines;
                    // Garder la dernière ligne (potentiellement incomplète) dans le buffer
                    if let Some(last) = lines.pop() {
                        buffer = last;
                    } else {
                        buffer.clear();
                    }

                    for line in lines {
                        if line.starts_with("data: ") {
                            let data = &line[6..]; // Supprimer "data: "
                            if data.trim().is_empty() {
                                // Ligne data vide, ignorer
                                continue;
                            }
                            if data.trim() == "[DONE]" {
                                stream_done = true;
                                break;
                            }
                            if self.debug {
                                println!("[Debug] SSE data: {}", data);
                            }
                            match serde_json::from_str::<ChatChunk>(data) {
                                Ok(chunk) => {
                                    for choice in chunk.choices {
                                        let delta = choice.delta;
                                        if let Some(content) = delta.content {
                                            // Afficher le contenu au fur et à mesure
                                            if !content.is_empty() {
                                                if !prefix_printed {
                                                    print!("{} ", "Agent:".blue().bold());
                                                    std::io::stdout().flush().ok();
                                                    prefix_printed = true;
                                                }
                                                print!("{}", content);
                                                std::io::stdout().flush().ok();
                                            }
                                            accumulated_message.content.push_str(&content);
                                        }
                                        if let Some(tool_calls) = delta.tool_calls {
                                            if self.debug {
                                                println!(
                                                    "[Debug] Tool calls delta: {:?}",
                                                    tool_calls
                                                );
                                            }
                                            for tool_call_delta in tool_calls {
                                                let index =
                                                    tool_call_delta.index.unwrap_or(0) as usize;
                                                // Étendre le vecteur si nécessaire
                                                while tool_call_builders.len() <= index {
                                                    tool_call_builders
                                                        .push(ToolCallBuilder::default());
                                                }
                                                tool_call_builders[index]
                                                    .update_from_delta(&tool_call_delta);

                                                // Vérifier si le builder est complet et le convertir
                                                if let Some(tool_call) =
                                                    tool_call_builders[index].to_tool_call()
                                                {
                                                    accumulated_tool_calls.push(tool_call);
                                                }
                                            }
                                        }
                                        if let Some(role) = delta.role {
                                            accumulated_message.role = role;
                                        }
                                    }
                                    // Si le chunk contient des infos d'usage, les sauvegarder
                                    if let Some(chunk_usage) = chunk.usage {
                                        usage = Some(chunk_usage);
                                    }
                                }
                                Err(e) => {
                                    if self.debug {
                                        println!("[Debug] Error parsing chunk: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    if self.debug {
                        println!("[Debug] Stream chunk error: {}", e);
                    }
                    // On continue malgré les erreurs de chunk
                }
            }
            // Vérifier si l'utilisateur a demandé une interruption
            if interrupt::check_interrupt() {
                if self.debug {
                    println!("[Debug] Interruption demandée par l'utilisateur");
                }
                interrupted = true;
                break;
            }
        }
        if interrupted {
            println!("\n[Interruption] Réponse interrompue par l'utilisateur.");
        } else if prefix_printed {
            println!(); // Nouvelle ligne après le streaming seulement si quelque chose a été affiché
        }

        // Si nous avons accumulé des tool_calls, les ajouter au message
        if !accumulated_tool_calls.is_empty() {
            accumulated_message.tool_calls = Some(accumulated_tool_calls);
        }

        // Créer une réponse factice avec le message accumulé
        let response = ChatResponse {
            choices: vec![Choice {
                message: accumulated_message,
            }],
            usage: usage.unwrap_or_else(|| Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                prompt_cache_hit_tokens: None,
                prompt_cache_miss_tokens: None,
            }),
        };

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_tool_call_builder_default() {
        let mut builder = ToolCallBuilder::default();
        assert!(builder.index.is_none());
        assert!(builder.id.is_none());
        assert!(builder.call_type.is_none());
        assert!(builder.function_name.is_none());
        assert!(builder.function_arguments.is_empty());
        assert!(!builder.converted);
        assert!(!builder.is_complete());
        assert!(builder.to_tool_call().is_none());
    }

    #[test]
    fn test_tool_call_builder_update_from_delta() {
        let mut builder = ToolCallBuilder::default();
        let delta = ToolCallDelta {
            index: Some(0),
            id: Some("call_123".to_string()),
            call_type: Some("function".to_string()),
            function: Some(FunctionCallDelta {
                name: Some("sh".to_string()),
                arguments: Some("{\"command\": \"ls\"}".to_string()),
            }),
        };
        builder.update_from_delta(&delta);
        assert_eq!(builder.index, Some(0));
        assert_eq!(builder.id, Some("call_123".to_string()));
        assert_eq!(builder.call_type, Some("function".to_string()));
        assert_eq!(builder.function_name, Some("sh".to_string()));
        assert_eq!(builder.function_arguments, "{\"command\": \"ls\"}".to_string());
        assert!(builder.is_complete());
        assert!(!builder.converted);
    }

    #[test]
    fn test_tool_call_builder_to_tool_call() {
        let mut builder = ToolCallBuilder::default();
        builder.id = Some("call_123".to_string());
        builder.call_type = Some("function".to_string());
        builder.function_name = Some("sh".to_string());
        builder.function_arguments = "{\"command\": \"ls\"}".to_string();
        assert!(builder.is_complete());
        let tool_call = builder.to_tool_call();
        assert!(tool_call.is_some());
        let tool_call = tool_call.unwrap();
        assert_eq!(tool_call.id, "call_123");
        assert_eq!(tool_call.call_type, "function");
        assert_eq!(tool_call.function.name, "sh");
        assert_eq!(tool_call.function.arguments, "{\"command\": \"ls\"}");
        // Après conversion, le builder est marqué comme converted
        assert!(builder.converted);
        // Et ne peut plus être reconverti
        assert!(builder.to_tool_call().is_none());
    }

    #[test]
    fn test_tool_call_builder_partial_updates() {
        let mut builder = ToolCallBuilder::default();
        // Première delta avec seulement l'index et l'ID
        let delta1 = ToolCallDelta {
            index: Some(0),
            id: Some("call_123".to_string()),
            call_type: None,
            function: None,
        };
        builder.update_from_delta(&delta1);
        assert!(builder.to_tool_call().is_none());
        // Deuxième delta avec le type
        let delta2 = ToolCallDelta {
            index: None,
            id: None,
            call_type: Some("function".to_string()),
            function: None,
        };
        builder.update_from_delta(&delta2);
        assert!(builder.to_tool_call().is_none());
        // Troisième delta avec le nom de fonction
        let delta3 = ToolCallDelta {
            index: None,
            id: None,
            call_type: None,
            function: Some(FunctionCallDelta {
                name: Some("sh".to_string()),
                arguments: None,
            }),
        };
        builder.update_from_delta(&delta3);
        assert!(builder.to_tool_call().is_none());
        // Quatrième delta avec les arguments (premier fragment)
        let delta4 = ToolCallDelta {
            index: None,
            id: None,
            call_type: None,
            function: Some(FunctionCallDelta {
                name: None,
                arguments: Some("{\"command\": ".to_string()),
            }),
        };
        builder.update_from_delta(&delta4);
        assert!(builder.to_tool_call().is_none()); // arguments pas encore valides JSON
        // Cinquième delta avec la suite des arguments
        let delta5 = ToolCallDelta {
            index: None,
            id: None,
            call_type: None,
            function: Some(FunctionCallDelta {
                name: None,
                arguments: Some("\"ls\"}".to_string()),
            }),
        };
        builder.update_from_delta(&delta5);
        let tool_call = builder.to_tool_call();
        assert!(tool_call.is_some()); // maintenant arguments valides JSON
        let tool_call = tool_call.unwrap();
        assert!(builder.converted); // marqué comme converti
        assert_eq!(tool_call.id, "call_123");
        assert_eq!(tool_call.call_type, "function");
        assert_eq!(tool_call.function.name, "sh");
        assert_eq!(tool_call.function.arguments, "{\"command\": \"ls\"}");
    }

    #[test]
    fn test_tool_call_builder_multiple_indexes() {
        let mut builders = vec![ToolCallBuilder::default(), ToolCallBuilder::default()];
        // Premier tool_call (index 0)
        let delta1 = ToolCallDelta {
            index: Some(0),
            id: Some("call_123".to_string()),
            call_type: Some("function".to_string()),
            function: Some(FunctionCallDelta {
                name: Some("sh".to_string()),
                arguments: Some("{\"command\": \"ls\"}".to_string()),
            }),
        };
        builders[0].update_from_delta(&delta1);
        assert!(builders[0].is_complete());
        // Deuxième tool_call (index 1)
        let delta2 = ToolCallDelta {
            index: Some(1),
            id: Some("call_456".to_string()),
            call_type: Some("function".to_string()),
            function: Some(FunctionCallDelta {
                name: Some("sh".to_string()),
                arguments: Some("{\"command\": \"pwd\"}".to_string()),
            }),
        };
        builders[1].update_from_delta(&delta2);
        assert!(builders[1].is_complete());
    }

    #[test]
    fn test_tool_call_builder_converted_prevents_updates() {
        let mut builder = ToolCallBuilder::default();
        builder.id = Some("call_123".to_string());
        builder.call_type = Some("function".to_string());
        builder.function_name = Some("sh".to_string());
        builder.function_arguments = "{\"command\": \"ls\"}".to_string();
        let _ = builder.to_tool_call(); // marque comme converted
        assert!(builder.converted);
        // Essayer de mettre à jour après conversion ne fait rien
        let delta = ToolCallDelta {
            index: Some(1),
            id: Some("call_456".to_string()),
            call_type: Some("function".to_string()),
            function: Some(FunctionCallDelta {
                name: Some("sh".to_string()),
                arguments: Some("{\"command\": \"pwd\"}".to_string()),
            }),
        };
        builder.update_from_delta(&delta);
        // Les champs restent inchangés
        assert_eq!(builder.id, Some("call_123".to_string()));
        assert_eq!(builder.call_type, Some("function".to_string()));
        assert_eq!(builder.function_name, Some("sh".to_string()));
        assert_eq!(builder.function_arguments, "{\"command\": \"ls\"}".to_string());
    }

    #[test]
    fn test_tool_call_builder_invalid_json() {
        let mut builder = ToolCallBuilder::default();
        builder.id = Some("call_123".to_string());
        builder.call_type = Some("function".to_string());
        builder.function_name = Some("sh".to_string());
        builder.function_arguments = "{\"command\": }".to_string(); // JSON invalide
        // is_complete retourne vrai car les champs sont présents et arguments non vides
        assert!(builder.is_complete());
        // Mais to_tool_call doit retourner None car JSON invalide
        assert!(builder.to_tool_call().is_none());
        // Le builder ne doit pas être marqué comme converted
        assert!(!builder.converted);
        // Si on corrige les arguments
        builder.function_arguments = "{\"command\": \"ls\"}".to_string();
        assert!(builder.to_tool_call().is_some());
        assert!(builder.converted);
    }

    // Note: les tests pour StreamProcessor.process_response nécessitent des mocks HTTP,
    // ce qui est plus complexe. On peut les ajouter plus tard avec un crate comme mockito.
    // Pour l'instant, on se concentre sur les tests unitaires des structures.
}
