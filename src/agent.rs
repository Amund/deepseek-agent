use rustyline;
use rustyline::error::ReadlineError;
use serde_json;

use crate::api::*;
use crate::api_client::ApiClient;
use crate::config::{DEFAULT_MAX_RETRIES, DEFAULT_MAX_RETRY_DELAY_MS, DEFAULT_RETRY_DELAY_MS};
use crate::history::HistoryManager;
use crate::interrupt;
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
        base_url: Option<String>,
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
                base_url,
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

    fn print_assistant_message(&self, msg: &Message) {
        if self.stream && !msg.content.is_empty() {
            // Le contenu a déjà été affiché via streaming, on n'affiche que si c'était vide
            // Mais on peut ajouter une nouvelle ligne si nécessaire
            // Le streaming ajoute déjà une nouvelle ligne à la fin
        } else if !msg.content.is_empty() {
            println!("Agent: {}", msg.content);
        }
    }

    async fn handle_assistant_message(&mut self, mut msg: Message) -> Result<(), Box<dyn std::error::Error>> {
        // Tant que l'assistant retourne des tool_calls, les exécuter
        loop {
            // Ajouter le message à l'historique
            self.history.add_message(msg.clone());

            // Vérifier si la session doit être redémarrée
            self.check_restart()?;

            // Afficher le contenu (si non vide) au cas où le streaming n'a pas été activé
            self.print_assistant_message(&msg);

            // Si le message n'a pas de tool_calls, on a fini
            if msg.tool_calls.is_none() {
                break;
            }

            // Prendre les tool_calls (consomme l'option)
            let tool_calls = msg.tool_calls.take().unwrap();
            let mut tool_results = Vec::new();
            let mut interrupted = false;

            for tool_call in tool_calls {
                // Vérifier si l'utilisateur a demandé une interruption
                if interrupt::check_interrupt() {
                    interrupted = true;
                    // Ajouter un message d'interruption à l'historique
                    self.history.add_message(Message {
                        role: "system".into(),
                        content: "Interruption demandée par l'utilisateur. Arrêt de l'exécution des commandes.".into(),
                        tool_calls: None,
                        tool_call_id: None,
                        token_count: None,
                    });
                    break;
                }
                if tool_call.function.name == "sh" {
                    // Vérifier que les arguments ne sont pas vides
                    if tool_call.function.arguments.is_empty() {
                        tool_results.push(Message {
                            role: "tool".into(),
                            content: "Erreur: arguments JSON vides pour l'outil 'sh'".into(),
                            tool_calls: None,
                            tool_call_id: Some(tool_call.id.clone()),
                            token_count: None,
                        });
                        continue;
                    }
                    // Extraire la commande des arguments JSON
                    let args: serde_json::Value = match serde_json::from_str(&tool_call.function.arguments) {
                        Ok(args) => args,
                        Err(e) => {
                            let error_msg = format!("Erreur de parsing JSON: {}. Arguments: {}", e, tool_call.function.arguments);
                            if self.debug {
                                println!("[Debug] {}", error_msg);
                            }
                            tool_results.push(Message {
                                role: "tool".into(),
                                content: error_msg,
                                tool_calls: None,
                                tool_call_id: Some(tool_call.id.clone()),
                                token_count: None,
                            });
                            continue;
                        }
                    };
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

            if interrupted {
                println!("[Interruption] Exécution des commandes interrompue.");
                break;
            }
            if tool_results.is_empty() {
                // Aucun résultat valide, on sort de la boucle
                break;
            }

            // Appel API pour obtenir la suite
            let response = self.call_api().await?;
            if let Some(choice) = response.choices.into_iter().next() {
                msg = choice.message;
                // Continuer la boucle pour traiter d'éventuels tool_calls
            } else {
                break;
            }
        }
        Ok(())
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
            // Vérifier si une interruption a été demandée (Ctrl+C ou Échap)
            if interrupt::is_interrupt_requested() {
                println!("\nInterruption détectée. Arrêt de l'agent.");
                break;
            }
            // Réinitialiser le flag d'interruption après la vérification
            interrupt::reset_interrupt();
            
            let user_input = match stdin.readline(">> ") {
                Ok(line) => line,
                Err(ReadlineError::Interrupted) => {
                    // Ctrl+C pendant la lecture de ligne
                    println!("\nInterruption. Arrêt de l'agent.");
                    break;
                }
                Err(ReadlineError::Eof) => {
                    // Ctrl+D (EOF)
                    println!("\nFin du fichier. Arrêt.");
                    break;
                }
                Err(err) => return Err(Box::new(err)),
            };
            
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
                // Gérer la réponse de l'assistant (tool_calls récursifs)
                self.handle_assistant_message(msg).await?;
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
#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_make_tools() {
        let tools = Agent::make_tools();
        assert_eq!(tools.len(), 1);
        let tool = &tools[0];
        assert_eq!(tool.tool_type, "function");
        assert_eq!(tool.function.name, "sh");
        assert_eq!(tool.function.description, "Exécute une commande shell bash");
        // Vérifier que les paramètres sont corrects
        let params = &tool.function.parameters;
        assert_eq!(params["type"], "object");
        assert_eq!(params["properties"]["command"]["type"], "string");
        assert_eq!(params["required"][0], "command");
    }

    #[test]
    fn test_agent_new() {
        let agent = Agent::new(
            "test_key".to_string(),
            Some("deepseek-chat".to_string()),
            Some("Test system prompt".to_string()),
            Some(vec!["ls".to_string()]),
            Some(vec!["rm".to_string()]),
            Some(10),
            Some(10000),
            false,
            Some(3),
            Some(100),
            Some(1000),
            Some(5000),
            Some(false),
            None,
        );
        assert_eq!(agent.model, "deepseek-chat");
        assert_eq!(agent.system_prompt, Some("Test system prompt".to_string()));
        assert!(!agent.debug);
        assert!(!agent.stream);
    }

    #[test]
    fn test_agent_new_defaults() {
        let agent = Agent::new(
            "test_key".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(agent.model, "deepseek-chat");
        assert!(agent.system_prompt.is_none());
        assert!(!agent.stream);
    }

    #[test]
    fn test_print_assistant_message() {
        // Note: cette fonction imprime sur stdout, difficile à tester unitairement.
        // On peut vérifier qu'elle ne panique pas.
        let agent = Agent::new(
            "test_key".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let msg = Message {
            role: "assistant".to_string(),
            content: "Hello".to_string(),
            tool_calls: None,
            tool_call_id: None,
            token_count: None,
        };
        // Appel de la fonction (ne devrait pas paniquer)
        agent.print_assistant_message(&msg);
    }

    #[test]
    fn test_check_restart() {
        // Cette fonction délègue à session::check_and_restart_if_needed
        // On peut tester avec un agent qui a un historique vide
        let agent = Agent::new(
            "test_key".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        // Avec un historique vide, should_restart_session retourne false
        // car max_context_tokens est None
        let result = agent.check_restart();
        assert!(result.is_ok());
    }
}
