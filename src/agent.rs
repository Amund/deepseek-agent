use bytes::Bytes;
use futures::StreamExt;
use reqwest::Client;
use rustyline;
use serde_json;
use std::io::Write;
use tokio::time::{sleep, Duration};

use crate::api::*;
use crate::config::{DEFAULT_MAX_RETRIES, DEFAULT_RETRY_DELAY_MS, DEFAULT_MAX_RETRY_DELAY_MS};
use crate::security::Security;
use crate::shell::ShellExecutor;
use crate::token_management::{estimate_message_tokens, estimate_tokens};

pub struct Agent {
    client: Client,
    api_key: String,
    model: String,
    system_prompt: Option<String>,
    messages: Vec<Message>,              // <-- historique en mémoire
    total_tokens: u32,                   // Total des tokens estimés dans l'historique
    security: Security,                  // Gestion de la sécurité
    shell_executor: ShellExecutor,       // Exécution shell
    max_history_messages: Option<usize>, // limite de messages (optionnel)
    max_context_tokens: Option<u32>,     // limite de tokens (optionnel, prioritaire)
    debug: bool,                         // mode debug
    max_retries: u32,
    retry_delay_ms: u64,
    max_retry_delay_ms: u64,
    token_calibration_factor: f32, // facteur pour ajuster les estimations
    total_real_tokens_observed: u32, // total des tokens réels observés (pour stats)
    total_estimated_tokens: u32,   // total des tokens estimés correspondants
    stream: bool,                  // streaming des réponses
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
        Agent {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| "deepseek-chat".to_string()),
            system_prompt,
            messages: Vec::new(),
            total_tokens: 0,
            security: Security::new(whitelist, blacklist),
            shell_executor: ShellExecutor::new(shell_timeout_ms),
            max_history_messages,
            max_context_tokens,
            debug,
            max_retries: max_retries.unwrap_or(DEFAULT_MAX_RETRIES),
            retry_delay_ms: retry_delay_ms.unwrap_or(DEFAULT_RETRY_DELAY_MS),
            max_retry_delay_ms: max_retry_delay_ms.unwrap_or(DEFAULT_MAX_RETRY_DELAY_MS),
            stream: stream.unwrap_or(false),
            token_calibration_factor: 1.0,
            total_real_tokens_observed: 0,
            total_estimated_tokens: 0,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Message système (personnalisable)
        let system_content = self.system_prompt.clone().unwrap_or_else(||
            "Tu es un assistant qui peut exécuter des commandes shell. Pour cela, utilise l'outil 'sh' avec le paramètre 'command'.".to_string()
        );

        self.add_message(Message {
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
            self.add_message(Message {
                role: "user".into(),
                content: user_input,
                tool_calls: None,
                tool_call_id: None,
                token_count: None,
            });

            // Appel API
            let response = self.call_deepseek().await?;

            if let Some(choice) = response.choices.into_iter().next() {
                let msg = choice.message;
                self.add_message(msg.clone());

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
                    // Ajouter tous les résultats à l'historique
                    for result in &tool_results {
                        self.add_message(result.clone());
                    }

                    // Si au moins un résultat a été généré, faire un appel API final
                    if !tool_results.is_empty() {
                        let final_response = self.call_deepseek().await?;
                        if let Some(final_choice) = final_response.choices.into_iter().next() {
                            let final_msg = final_choice.message;
                            println!("Agent: {}", final_msg.content);
                            self.add_message(final_msg);
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

    async fn call_deepseek(&mut self) -> Result<ChatResponse, Box<dyn std::error::Error>> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: self.messages.clone(),
            tools: vec![Tool {
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
            }],
            tool_choice: "auto".into(),
            stream: self.stream,
        };

        let mut last_error: Option<Box<dyn std::error::Error>> = None;

        for attempt in 0..=self.max_retries {
            match self
                .client
                .post("https://api.deepseek.com/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&request)
                .send()
                .await
            {
                Ok(resp) => {
                    // Vérifier le statut HTTP
                    if resp.status().is_success() {
                        match self.process_response(resp, &request).await {
                            Ok(response) => {
                                return Ok(response);
                            }
                            Err(e) => {
                                last_error = Some(e);
                                if self.debug {
                                    println!(
                                        "[Debug] Error processing response on attempt {}: {}",
                                        attempt,
                                        last_error.as_ref().unwrap()
                                    );
                                }
                            }
                        }
                    } else {
                        // Erreur HTTP (429, 500, etc.)
                        let status = resp.status();
                        let text = resp.text().await.unwrap_or_default();
                        last_error = Some(format!("HTTP {}: {}", status, text).into());
                        if self.debug {
                            println!(
                                "[Debug] HTTP error on attempt {}: {}",
                                attempt,
                                last_error.as_ref().unwrap()
                            );
                        }

                        // Si c'est une erreur client (4xx) sauf 429 (rate limit), on ne retry pas
                        if status.is_client_error() && status != 429 {
                            break;
                        }
                        // Pour les autres erreurs (429, 5xx), on retry
                    }
                }
                Err(e) => {
                    last_error = Some(e.into());
                    if self.debug {
                        println!(
                            "[Debug] Network error on attempt {}: {}",
                            attempt,
                            last_error.as_ref().unwrap()
                        );
                    }
                }
            }

            // Si ce n'est pas la dernière tentative, attendre avant de retry
            if attempt < self.max_retries {
                // Backoff exponentiel avec délai maximum
                let delay_ms = std::cmp::min(
                    self.retry_delay_ms * 2u64.pow(attempt),
                    self.max_retry_delay_ms,
                );
                if self.debug {
                    println!("[Debug] Retrying in {} ms...", delay_ms);
                }
                sleep(Duration::from_millis(delay_ms)).await;
            }
        }

        // Si on arrive ici, toutes les tentatives ont échoué
        Err(last_error.unwrap_or_else(|| "Unknown error".into()))
    }

    // Traite une réponse streaming ou non-streaming
    async fn process_response(
        &mut self,
        resp: reqwest::Response,
        request: &ChatRequest,
    ) -> Result<ChatResponse, Box<dyn std::error::Error>> {
        if self.stream {
            // Mode streaming
            let mut accumulated_message = Message {
                role: "assistant".to_string(),
                content: String::new(),
                tool_calls: None,
                tool_call_id: None,
                token_count: None,
            };
            // Pour construire les tool_calls en streaming
            #[derive(Debug, Default)]
            struct ToolCallBuilder {
                index: Option<u32>,
                id: Option<String>,
                call_type: Option<String>,
                function_name: Option<String>,
                function_arguments: Option<String>,
                converted: bool,
            }

            impl ToolCallBuilder {
                fn is_complete(&self) -> bool {
                    !self.converted
                        && self.id.is_some()
                        && self.call_type.is_some()
                        && self.function_name.is_some()
                        && self.function_arguments.is_some()
                }

                fn to_tool_call(&mut self) -> Option<ToolCall> {
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

                fn update_from_delta(&mut self, delta: &ToolCallDelta) {
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

                fn default() -> Self {
                    ToolCallBuilder {
                        index: None,
                        id: None,
                        call_type: None,
                        function_name: None,
                        function_arguments: None,
                        converted: false,
                    }
                }
            }

            let mut tool_call_builders: Vec<ToolCallBuilder> = Vec::new();
            let mut accumulated_tool_calls: Vec<ToolCall> = Vec::new();
            let mut usage: Option<Usage> = None;

            // Lire le stream ligne par ligne
            let stream = resp.bytes_stream();
            tokio::pin!(stream);
            let mut buffer = String::new();
            let mut stream_done = false;
            // START STREAMING WHILE BLOCK
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
                                                print!("{}", content);
                                                std::io::stdout().flush().ok();
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
            }
            // END STREAMING WHILE BLOCK
            println!(); // Nouvelle ligne après le streaming

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

            // Calibration des tokens (si usage disponible)
            self.calibrate_with_response(request, &response);
            Ok(response)
        } else {
            // Mode non-streaming
            let response = resp.json::<ChatResponse>().await?;
            self.calibrate_with_response(request, &response);
            Ok(response)
        }
    }

    // Calibre les estimations de tokens basées sur la réponse réelle de l'API
    fn calibrate_with_response(&mut self, request: &ChatRequest, response: &ChatResponse) {
        let real_prompt_tokens = response.usage.prompt_tokens;

        // Calculer les tokens bruts des messages (sans calibration)
        let raw_message_tokens: u32 = self
            .messages
            .iter()
            .map(|msg| {
                // Si le message a déjà un token_count, c'est déjà calibré
                // On doit recalculer les tokens bruts
                // Pour simplifier, on utilise estimate_message_tokens qui donne une estimation brute
                estimate_message_tokens(msg)
            })
            .sum();

        // Estimer les tokens des définitions d'outils (bruts)
        let tools_json = serde_json::to_string(&request.tools).unwrap_or_default();
        let raw_tools_tokens = estimate_tokens(&tools_json);

        // Tokens supplémentaires (metadata)
        const EXTRA_TOKENS: u32 = 50;

        let total_raw_estimated = raw_message_tokens + raw_tools_tokens + EXTRA_TOKENS;

        // Mettre à jour les statistiques
        self.total_real_tokens_observed += real_prompt_tokens;
        self.total_estimated_tokens += total_raw_estimated;

        // Afficher les statistiques de cache si disponibles
        if self.debug {
            if let Some(cache_hit) = response.usage.prompt_cache_hit_tokens {
                if let Some(cache_miss) = response.usage.prompt_cache_miss_tokens {
                    let cache_hit_ratio = if real_prompt_tokens > 0 {
                        cache_hit as f32 / real_prompt_tokens as f32
                    } else {
                        0.0
                    };
                    println!(
                        "[Debug] Cache stats: hit={} ({}%), miss={}, total={}",
                        cache_hit,
                        (cache_hit_ratio * 100.0).round(),
                        cache_miss,
                        real_prompt_tokens
                    );
                }
            }
        }

        // Calculer l'erreur relative
        if real_prompt_tokens > 0 && total_raw_estimated > 0 {
            let error = real_prompt_tokens as f32 / total_raw_estimated as f32;

            // Ajuster le facteur de calibration si l'erreur est significative
            // On utilise une moyenne mobile exponentielle avec facteur 0.1
            const SMOOTHING_FACTOR: f32 = 0.1;
            const SIGNIFICANT_ERROR: f32 = 0.1; // 10%

            if (error - 1.0).abs() > SIGNIFICANT_ERROR {
                self.token_calibration_factor = self.token_calibration_factor
                    * (1.0 - SMOOTHING_FACTOR)
                    + error * SMOOTHING_FACTOR;

                // Limiter le facteur à une plage raisonnable (0.5 à 2.0)
                self.token_calibration_factor = self.token_calibration_factor.clamp(0.5, 2.0);

                if self.debug {
                    println!(
                        "[Debug] Token calibration: real={}, raw_est={}, error={:.2}%, new_factor={:.3}",
                        real_prompt_tokens,
                        total_raw_estimated,
                        (error - 1.0) * 100.0,
                        self.token_calibration_factor
                    );
                }
            } else if self.debug {
                println!(
                    "[Debug] Token estimation accurate: real={}, raw_est={}, error={:.2}%",
                    real_prompt_tokens,
                    total_raw_estimated,
                    (error - 1.0) * 100.0
                );
            }
        }
    }


    // Valide une commande shell par rapport aux listes blanche/noire et règles de sécurité

    // Met à jour l'estimation de tokens pour un message
    fn estimate_and_set_tokens(&mut self, message: &mut Message) -> u32 {
        if message.token_count.is_none() {
            let raw_tokens = estimate_message_tokens(message);
            // Appliquer le facteur de calibration
            let calibrated_tokens = std::cmp::max(
                1,
                (raw_tokens as f32 * self.token_calibration_factor).round() as u32,
            );
            message.token_count = Some(calibrated_tokens);
            calibrated_tokens
        } else {
            message.token_count.unwrap_or(0)
        }
    }

    // Ajoute un message à l'historique en respectant les limites
    fn add_message(&mut self, mut message: Message) {
        // Estimer et stocker les tokens du message
        let message_tokens = self.estimate_and_set_tokens(&mut message);
        self.total_tokens += message_tokens;

        if self.debug {
            println!(
                "[Debug] Adding message: {} tokens, total: {}",
                message_tokens, self.total_tokens
            );
        }

        self.messages.push(message);

        // Appliquer les limites
        self.trim_to_limits();
    }

    // Supprime un bloc de messages et met à jour le compteur de tokens
    fn remove_messages(&mut self, range: std::ops::Range<usize>) {
        let removed_tokens: u32 = self.messages[range.clone()]
            .iter()
            .map(|msg| msg.token_count.unwrap_or(0))
            .sum();

        self.total_tokens -= removed_tokens;
        self.messages.drain(range);

        if self.debug {
            println!(
                "[Debug] Removed {} tokens, new total: {}",
                removed_tokens, self.total_tokens
            );
        }
    }

    // Applique toutes les limites (messages et tokens)
    fn trim_to_limits(&mut self) {
        let mut needs_trimming = false;

        // Vérifier la limite de messages
        if let Some(max_messages) = self.max_history_messages {
            if self.messages.len() > max_messages {
                if self.debug {
                    println!(
                        "[Debug] Exceeded message limit: {} > {}",
                        self.messages.len(),
                        max_messages
                    );
                }
                needs_trimming = true;
            }
        }

        // Vérifier la limite de tokens
        if let Some(max_tokens) = self.max_context_tokens {
            if self.total_tokens > max_tokens {
                if self.debug {
                    println!(
                        "[Debug] Exceeded token limit: {} > {}",
                        self.total_tokens, max_tokens
                    );
                }
                needs_trimming = true;
            }
        }

        if needs_trimming {
            self.trim_history_smart();
        }
    }

    // Tronque l'historique de manière intelligente pour optimiser le cache KV
    // Prend en compte à la fois les limites de messages et de tokens
    fn trim_history_smart(&mut self) {
        // Rien à faire si pas de limites
        if self.max_history_messages.is_none() && self.max_context_tokens.is_none() {
            return;
        }

        // Calculer combien il faut supprimer pour respecter les limites
        let mut messages_to_remove = 0;
        let mut tokens_to_remove = 0;

        // Limite de messages
        if let Some(max_messages) = self.max_history_messages {
            if self.messages.len() > max_messages {
                messages_to_remove = self.messages.len() - max_messages;
            }
        }

        // Limite de tokens
        if let Some(max_tokens) = self.max_context_tokens {
            if self.total_tokens > max_tokens {
                tokens_to_remove = self.total_tokens - max_tokens;
            }
        }

        if messages_to_remove == 0 && tokens_to_remove == 0 {
            return; // Dans les limites
        }

        if self.debug {
            println!(
                "[Debug] Need to remove: {} messages and/or {} tokens",
                messages_to_remove, tokens_to_remove
            );
            println!(
                "[Debug] Current: {} messages, {} tokens",
                self.messages.len(),
                self.total_tokens
            );
        }

        // Stratégie : déterminer combien de messages supprimer
        // On doit supprimer au moins assez pour respecter la limite de messages
        // et aussi assez pour respecter la limite de tokens
        let mut min_messages_to_remove = messages_to_remove;

        // Si on a une limite de tokens, calculer combien de messages il faut supprimer
        // pour atteindre cette limite
        if tokens_to_remove > 0 {
            let mut accumulated_tokens = 0;
            let mut messages_needed_for_tokens = 0;

            // Parcourir les messages du début (après système) pour estimer
            // combien en supprimer pour atteindre tokens_to_remove
            let system_count = if !self.messages.is_empty() && self.messages[0].role == "system" {
                1
            } else {
                0
            };

            for i in system_count..self.messages.len() {
                if accumulated_tokens >= tokens_to_remove {
                    break;
                }
                accumulated_tokens += self.messages[i].token_count.unwrap_or(0);
                messages_needed_for_tokens += 1;
            }

            // Prendre le maximum entre messages_to_remove et messages_needed_for_tokens
            min_messages_to_remove =
                std::cmp::max(min_messages_to_remove, messages_needed_for_tokens);
        }

        if min_messages_to_remove == 0 {
            return;
        }

        // Identifier le message système
        let system_count = if !self.messages.is_empty() && self.messages[0].role == "system" {
            1
        } else {
            0
        };

        // Paramètres d'optimisation du cache
        const PROTECTED_RATIO: f32 = 0.25; // 25% des messages protégés
        const MIN_PROTECTED_MESSAGES: usize = 3; // Au moins système + 2 autres

        // Taille du segment protégé
        let total_messages = self.messages.len();
        let mut protected_size = std::cmp::max(
            system_count + MIN_PROTECTED_MESSAGES.saturating_sub(1), // système + (MIN-1) autres
            (total_messages as f32 * PROTECTED_RATIO).ceil() as usize,
        );
        protected_size = protected_size.min(total_messages - 2); // Garder au moins 2 messages à la fin

        // Où commencer la suppression ? Après le segment protégé si possible
        let delete_start_index = protected_size;

        // Où terminer la suppression ? On essaie de supprimer min_messages_to_remove messages
        let mut delete_end_index = delete_start_index + min_messages_to_remove;

        // S'assurer qu'on ne supprime pas les derniers messages
        let min_keep_at_end = 2;
        let max_delete_index = total_messages - min_keep_at_end;

        if delete_start_index >= max_delete_index {
            // Cas rare : segment protégé trop grand, supprimer à la fin
            let delete_end = total_messages - min_keep_at_end;
            let delete_start = delete_end.saturating_sub(min_messages_to_remove);
            if delete_start < delete_end {
                if self.debug {
                    println!(
                        "[Debug] Deleting from end: messages[{}..{}] (protected segment too large)",
                        delete_start, delete_end
                    );
                }
                self.remove_messages(delete_start..delete_end);
            }
        } else {
            // Limiter la suppression à max_delete_index
            delete_end_index = delete_end_index.min(max_delete_index);

            if delete_end_index > delete_start_index {
                // Essayer d'ajuster pour supprimer sur une frontière de conversation
                let adjusted_end = self.find_better_boundary(delete_start_index, delete_end_index);

                if adjusted_end > delete_start_index && adjusted_end <= max_delete_index {
                    if self.debug {
                        println!(
                            "[Debug] Deleting with adjusted boundary: messages[{}..{}]",
                            delete_start_index, adjusted_end
                        );
                    }
                    self.remove_messages(delete_start_index..adjusted_end);
                } else if delete_end_index > delete_start_index {
                    if self.debug {
                        println!(
                            "[Debug] Deleting: messages[{}..{}]",
                            delete_start_index, delete_end_index
                        );
                    }
                    self.remove_messages(delete_start_index..delete_end_index);
                }
            }
        }

        // Si après suppression on est encore au-dessus des limites
        // (peut arriver si on n'a pas pu supprimer assez), tronquer les plus anciens
        // (après le système) en dernier recours
        let mut still_needs_trimming = false;

        if let Some(max_messages) = self.max_history_messages {
            if self.messages.len() > max_messages {
                still_needs_trimming = true;
            }
        }

        if let Some(max_tokens) = self.max_context_tokens {
            if self.total_tokens > max_tokens {
                still_needs_trimming = true;
            }
        }

        if still_needs_trimming && self.messages.len() > system_count + 1 {
            let excess_messages = self.messages.len()
                - std::cmp::max(self.max_history_messages.unwrap_or(0), system_count + 1);
            let excess_tokens = if self.max_context_tokens.is_some() {
                self.total_tokens
                    .saturating_sub(self.max_context_tokens.unwrap())
            } else {
                0
            };

            // Supprimer assez de messages pour respecter les deux limites
            let mut to_remove = excess_messages;
            if excess_tokens > 0 {
                // Calculer combien de messages supplémentaires supprimer pour les tokens
                let mut token_count = 0;
                let mut additional_messages = 0;
                for i in system_count..self.messages.len() {
                    if token_count >= excess_tokens {
                        break;
                    }
                    token_count += self.messages[i].token_count.unwrap_or(0);
                    additional_messages += 1;
                }
                to_remove = std::cmp::max(to_remove, additional_messages);
            }

            if to_remove > 0 && self.messages.len() > system_count + to_remove {
                if self.debug {
                    println!(
                        "[Debug] Fallback deletion: messages[{}..{}]",
                        system_count,
                        system_count + to_remove
                    );
                }
                self.remove_messages(system_count..system_count + to_remove);
            }
        }
    }

    // Trouve une meilleure frontière pour la suppression (optimisation cache)
    fn find_better_boundary(&self, start: usize, end: usize) -> usize {
        if end >= self.messages.len() {
            return self.messages.len();
        }

        // Chercher une frontière naturelle près de 'end'
        // Préférer : après un message assistant (sans tool_calls) ou avant un user

        // Chercher vers l'avant depuis end
        for i in end..self.messages.len() {
            if i > 0 && i - 1 >= start {
                let prev = &self.messages[i - 1];
                // Bonne frontière : après un assistant sans tool_calls
                if prev.role == "assistant" && prev.tool_calls.is_none() {
                    return i;
                }
            }
            // Bonne frontière : avant un user
            if self.messages[i].role == "user" && i > start {
                return i;
            }
        }

        // Chercher vers l'arrière depuis end
        for i in (start + 1..=end.min(self.messages.len() - 1)).rev() {
            if i > 0 {
                let prev = &self.messages[i - 1];
                if prev.role == "assistant" && prev.tool_calls.is_none() {
                    return i;
                }
            }
            if self.messages[i].role == "user" && i > start {
                return i;
            }
        }

        // Retourner la position originale
        end
    }
}

