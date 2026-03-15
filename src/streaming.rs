use crate::api::*;
use crate::interrupt;
use futures::StreamExt;
use reqwest::Response;
use std::io::Write;



// Pour construire les tool_calls en streaming
#[derive(Debug, Default)]
pub struct ToolCallBuilder {
    index: Option<u32>,
    id: Option<String>,
    call_type: Option<String>,
    function_name: Option<String>,
    function_arguments: Option<String>,
    converted: bool,
}

impl ToolCallBuilder {
    pub fn is_complete(&self) -> bool {
        !self.converted
            && self.id.is_some()
            && self.call_type.is_some()
            && self.function_name.is_some()
            && self.function_arguments.is_some()
    }

    pub fn to_tool_call(&mut self) -> Option<ToolCall> {
        if !self.is_complete() {
            return None;
        }
        self.converted = true;
        Some(ToolCall {
            id: self.id.clone().unwrap(),
            call_type: self.call_type.clone().unwrap(),
            function: FunctionCall {
                name: self.function_name.clone().unwrap(),
                arguments: self.function_arguments.clone().unwrap(),
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
                self.function_arguments = Some(arguments.clone());
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
                                                    print!("Agent: ");
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
        } else {
            println!(); // Nouvelle ligne après le streaming
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
