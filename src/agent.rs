use rustyline;
use serde_json;

use crate::api::*;
use crate::api_client::ApiClient;
use crate::config::{DEFAULT_MAX_RETRIES, DEFAULT_MAX_RETRY_DELAY_MS, DEFAULT_RETRY_DELAY_MS};
use crate::history::HistoryManager;
use crate::security::Security;
use crate::session::{self, RestartSessionError};
use crate::shell::ShellExecutor;

pub struct Agent {
    api_client: ApiClient,
    history: HistoryManager,
    system_prompt: Option<String>,
    security: Security,
    shell_executor: ShellExecutor,
    model: String,
    debug: bool,
    stream: bool,
}

impl Agent {
    pub fn new(
        api_key: String,
        model: Option<String>,
        system_prompt: Option<String>,
        whitelist: Option<Vec<String>>,
        blacklist: Option<Vec<String>>,
        max_history_messages: Option<usize>,
        max_context_tokens: Option<u32>,
        debug: bool,
        max_retries: Option<u32>,
        retry_delay_ms: Option<u64>,
        max_retry_delay_ms: Option<u64>,
        shell_timeout_ms: Option<u64>,
        stream: Option<bool>,
    ) -> Self {
        let stream_enabled = stream.unwrap_or(false);
        let max_retries_val = max_retries.unwrap_or(DEFAULT_MAX_RETRIES);
        let retry_delay_ms_val = retry_delay_ms.unwrap_or(DEFAULT_RETRY_DELAY_MS);
        let max_retry_delay_ms_val = max_retry_delay_ms.unwrap_or(DEFAULT_MAX_RETRY_DELAY_MS);

        let model_str = model.unwrap_or_else(|| "deepseek-chat".to_string());
        Agent {
            api_client: ApiClient::new(
                api_key,
                Some(model_str.clone()),
                stream_enabled,
                debug,
                max_retries_val,
                retry_delay_ms_val,
                max_retry_delay_ms_val,
            ),
            history: HistoryManager::new(max_history_messages, max_context_tokens, debug),
            system_prompt,
            security: Security::new(whitelist, blacklist),
            shell_executor: ShellExecutor::new(shell_timeout_ms),
            model: model_str,
            debug,
            stream: stream_enabled,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Message système (personnalisable)
        let system_content = self.system_prompt.clone().unwrap_or_else(||
            "Tu es un assistant qui peut exécuter des commandes shell. Pour cela, utilise l'outil 'sh' avec le paramètre 'command'.".to_string()
        );

        self.history.add_message(Message {
            role: "system".into(),
            content: system_content,
            tool_calls: None,
            tool_call_id: None,
            token_count: None,
        });

        println!("Agent DeepSeek. Tapez 'quit' pour sortir.");
        let mut stdin = rustyline::DefaultEditor::new()?;

        loop {
            let user_input = stdin.readline(">> ")?;
            if user_input == "quit" {
                break;
            }

            // Ajout du message utilisateur à l'historique
            self.history.add_message(Message {
                role: "user".into(),
                content: user_input,
                tool_calls: None,
                tool_call_id: None,
                token_count: None,
            });

            // Vérifier si la session doit être redémarrée
            self.check_restart()?;

            // Appel API
            let response = self.call_api().await?;

            if let Some(choice) = response.choices.into_iter().next() {
                let msg = choice.message;
                self.history.add_message(msg.clone());

                // Vérifier si la session doit être redémarrée
                self.check_restart()?;

                // Si l'assistant demande un outil
                if let Some(tool_calls) = msg.tool_calls {
                    let mut tool_results = Vec::new();

                    for tool_call in tool_calls {
                        if tool_call.function.name == "sh" {
                            // Extraire la commande des arguments JSON
                            let args: serde_json::Value =
                                serde_json::from_str(&tool_call.function.arguments)?;
                            let command = args["command"].as_str().unwrap_or("").to_string();

                            // Vérification de sécurité
                            if let Err(error_msg) = self.security.validate_command(&command) {
                                tool_results.push(Message {
                                    role: "tool".into(),
                                    content: error_msg,
                                    tool_calls: None,
                                    tool_call_id: Some(tool_call.id.clone()),
                                    token_count: None,
                                });
                                continue;
                            }

                            println!("[Shell] Exécution : {}", command);
                            let output = self.shell_executor.exec(&command).await;

                            // Ajouter le résultat à la liste
                            tool_results.push(Message {
                                role: "tool".into(),
                                content: output,
                                tool_calls: None,
                                tool_call_id: Some(tool_call.id.clone()),
                                token_count: None,
                            });
                        }
                    }

                    // Ajouter tous les résultats à l'historique
                    for result in &tool_results {
                        self.history.add_message(result.clone());
                    }

                    // Vérifier si la session doit être redémarrée
                    self.check_restart()?;

                    // Si au moins un résultat a été généré, faire un appel API final
                    if !tool_results.is_empty() {
                        let final_response = self.call_api().await?;
                        if let Some(final_choice) = final_response.choices.into_iter().next() {
                            let final_msg = final_choice.message;
                            println!("Agent: {}", final_msg.content);
                            self.history.add_message(final_msg);

                            // Vérifier si la session doit être redémarrée
                            self.check_restart()?;
                        }
                    }
                } else {
                    // Réponse textuelle normale
                    println!("Agent: {}", msg.content);
                }
            }
        }
        Ok(())
    }

    fn check_restart(&self) -> Result<(), RestartSessionError> {
        session::check_and_restart_if_needed(
            self.history.should_restart_session(),
            &self.history.messages,
            self.debug,
        )
    }

    fn make_tools() -> Vec<Tool> {
        vec![Tool {
            tool_type: "function".into(),
            function: ToolFunction {
                name: "sh".into(),
                description: "Exécute une commande shell bash".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Commande shell à exécuter"
                        }
                    },
                    "required": ["command"]
                }),
            },
        }]
    }

    async fn call_api(&mut self) -> Result<ChatResponse, Box<dyn std::error::Error>> {
        let tools = Self::make_tools();
        let request = ChatRequest {
            model: self.model.clone(),
            messages: self.history.messages.clone(),
            tools,
            tool_choice: "auto".into(),
            stream: self.stream,
        };
        let response = self.api_client.call(&request).await?;
        // Calibration des tokens
        self.history.calibrate_with_response(&request, &response);
        Ok(response)
    }
}
