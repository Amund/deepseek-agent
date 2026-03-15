use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

use tokio::process::Command;
use tokio::time::{sleep, timeout, Duration};

// Constantes pour la gestion des erreurs et retries
const DEFAULT_MAX_RETRIES: u32 = 3;
const DEFAULT_RETRY_DELAY_MS: u64 = 1000;
const DEFAULT_MAX_RETRY_DELAY_MS: u64 = 30000;

// ------------------ Structures pour l'API DeepSeek ------------------

// Fonction pour estimer le nombre de tokens dans un texte
// Estimation basée sur la longueur (approximation)
// En pratique, utiliser les retours de l'API pour plus de précision
fn estimate_tokens(text: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }

    // Estimation conservatrice :
    // - Pour l'anglais : ~1 token pour 4 caractères
    // - Pour d'autres langues (français, etc.) : ~1 token pour 3 caractères
    // - Pour le code/commandes shell : variable
    // On prend 1 token pour 3 caractères pour être prudent

    let char_count = text.chars().count() as u32;

    // Token minimum pour un texte non vide
    std::cmp::max(1, char_count / 3)
}

fn estimate_message_tokens(message: &Message) -> u32 {
    let mut total = estimate_tokens(&message.content);

    // Ajouter les tokens pour les tool_calls si présents
    if let Some(tool_calls) = &message.tool_calls {
        for tool_call in tool_calls {
            // Estimer les tokens pour l'ID, le type, et les arguments
            total += estimate_tokens(&tool_call.id);
            total += estimate_tokens(&tool_call.call_type);
            total += estimate_tokens(&tool_call.function.name);
            total += estimate_tokens(&tool_call.function.arguments);
        }
    }

    // Ajouter les tokens pour tool_call_id si présent
    if let Some(tool_call_id) = &message.tool_call_id {
        total += estimate_tokens(tool_call_id);
    }

    // Ajouter les tokens pour le rôle
    total += estimate_tokens(&message.role);

    total
}

// Fonction pour déterminer la limite de tokens par défaut selon le modèle
fn default_max_context_tokens_for_model(model: &Option<String>) -> u32 {
    let model_name = model.as_deref().unwrap_or("deepseek-chat");

    // Basé sur la documentation DeepSeek :
    // - deepseek-chat: 128K tokens input, jusqu'à 8K tokens output
    // - deepseek-reasoner: 128K tokens input, jusqu'à 64K tokens (32K max de reasoning) output
    // Pour les autres modèles, on suppose une limite conservatrice de 32K

    match model_name {
        "deepseek-chat" => {
            // 128K input - réserve pour la sortie (8K) et les tokens système
            const RESERVED_FOR_OUTPUT: u32 = 12_000; // 8K sortie max + 4K marge
            const SYSTEM_TOKENS: u32 = 4_000; // tokens système, outils, etc.
            128_000 - RESERVED_FOR_OUTPUT - SYSTEM_TOKENS // 112K tokens
        }
        "deepseek-reasoner" => {
            // 128K input - réserve pour la sortie (64K max, mais raisonnement 32K)
            // On réserve plus pour permettre des réponses longues
            const RESERVED_FOR_OUTPUT: u32 = 20_000; // raisonnement long possible
            const SYSTEM_TOKENS: u32 = 4_000;
            128_000 - RESERVED_FOR_OUTPUT - SYSTEM_TOKENS // 104K tokens
        }
        _ => {
            // Modèles plus anciens ou inconnus - limite conservatrice
            // On suppose 32K tokens maximum avec marge
            const RESERVED_FOR_OUTPUT: u32 = 4_000;
            32_000 - RESERVED_FOR_OUTPUT // 28K tokens (compatible avec l'ancienne valeur)
        }
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String, // "system", "user", "assistant", "tool"
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip)]
    token_count: Option<u32>, // Estimation ou comptage réel
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String, // "function"
    function: FunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct FunctionCall {
    name: String,
    arguments: String, // string JSON
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    tools: Vec<Tool>,
    tool_choice: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Debug, Serialize)]
struct Tool {
    #[serde(rename = "type")]
    tool_type: String,
    function: ToolFunction,
}

#[derive(Debug, Serialize)]
struct ToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    #[allow(dead_code)]
    completion_tokens: u32,
    #[allow(dead_code)]
    total_tokens: u32,
    prompt_cache_hit_tokens: Option<u32>,
    prompt_cache_miss_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

// Structures pour le streaming
#[derive(Debug, Deserialize)]
struct ChatChunk {
    choices: Vec<ChunkChoice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCallDelta>>,
}

// Structures pour les tool_calls streaming (champs optionnels)
#[derive(Debug, Deserialize, Clone)]
struct ToolCallDelta {
    #[serde(default)]
    index: Option<u32>, // index dans le tableau tool_calls
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type", default)]
    call_type: Option<String>,
    #[serde(default)]
    function: Option<FunctionCallDelta>,
}

#[derive(Debug, Deserialize, Clone)]
struct FunctionCallDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

// ------------------ Agent ------------------
struct Agent {
    client: Client,
    api_key: String,
    model: String,
    system_prompt: Option<String>,
    messages: Vec<Message>,              // <-- historique en mémoire
    total_tokens: u32,                   // Total des tokens estimés dans l'historique
    whitelist: Option<Vec<String>>,      // optionnel, pour restreindre les commandes
    blacklist: Option<Vec<String>>,      // optionnel, pour interdire des commandes
    max_history_messages: Option<usize>, // limite de messages (optionnel)
    max_context_tokens: Option<u32>,     // limite de tokens (optionnel, prioritaire)
    debug: bool,                         // mode debug
    max_retries: u32,
    retry_delay_ms: u64,
    max_retry_delay_ms: u64,
    shell_timeout_ms: Option<u64>,
    token_calibration_factor: f32, // facteur pour ajuster les estimations
    total_real_tokens_observed: u32, // total des tokens réels observés (pour stats)
    total_estimated_tokens: u32,   // total des tokens estimés correspondants
    stream: bool,                  // streaming des réponses
}

impl Agent {
    fn new(
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
            whitelist,
            blacklist,
            max_history_messages,
            max_context_tokens,
            debug,
            max_retries: max_retries.unwrap_or(DEFAULT_MAX_RETRIES),
            retry_delay_ms: retry_delay_ms.unwrap_or(DEFAULT_RETRY_DELAY_MS),
            max_retry_delay_ms: max_retry_delay_ms.unwrap_or(DEFAULT_MAX_RETRY_DELAY_MS),
            shell_timeout_ms,
            stream: stream.unwrap_or(false),
            token_calibration_factor: 1.0,
            total_real_tokens_observed: 0,
            total_estimated_tokens: 0,
        }
    }

    async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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

        println!("Agent DeepSeek minimal (Docker). Tapez 'quit' pour sortir.");
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
                            if let Err(error_msg) = self.validate_command(&command) {
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
                            let output = self.exec_shell(&command).await;

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
                    Ok(bytes) => {
                        let chunk_str = String::from_utf8_lossy(&bytes);
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

    async fn exec_shell(&self, command: &str) -> String {
        // Exécution avec timeout optionnel
        let cmd_future = Command::new("sh").arg("-c").arg(command).output();

        let output = if let Some(timeout_ms) = self.shell_timeout_ms {
            match timeout(Duration::from_millis(timeout_ms), cmd_future).await {
                Ok(result) => result,
                Err(_) => {
                    return format!(
                        "Timeout: la commande a dépassé le temps imparti ({} ms)",
                        timeout_ms
                    );
                }
            }
        } else {
            cmd_future.await
        };

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                if !stderr.is_empty() {
                    format!("STDERR:\n{}\nSTDOUT:\n{}", stderr, stdout)
                } else {
                    stdout
                }
            }
            Err(e) => format!("Erreur d'exécution : {}", e),
        }
    }

    // Valide une commande shell par rapport aux listes blanche/noire et règles de sécurité
    fn validate_command(&self, command: &str) -> Result<(), String> {
        let cmd_name = command.split_whitespace().next().unwrap_or("");

        // Vérification liste noire (prioritaire) - sur tous les tokens
        if let Some(blacklist) = &self.blacklist {
            // Vérifier le premier mot
            if blacklist.contains(&cmd_name.to_string()) {
                return Err(format!("Commande '{}' interdite (liste noire)", cmd_name));
            }
            // Vérifier tous les tokens pour plus de sécurité
            for token in command.split_whitespace() {
                // Ignorer les tokens qui sont des options (commencent par -)
                if !token.starts_with('-') && blacklist.contains(&token.to_string()) {
                    return Err(format!(
                        "Token '{}' interdit dans la commande (liste noire)",
                        token
                    ));
                }
            }
        }

        // Vérification liste blanche (seulement sur le premier mot)
        if let Some(whitelist) = &self.whitelist {
            if !whitelist.contains(&cmd_name.to_string()) {
                return Err(format!(
                    "Commande '{}' non autorisée (liste blanche)",
                    cmd_name
                ));
            }
        }

        // Validation de sécurité basique
        let lower_command = command.to_lowercase();
        let dangerous_patterns = [
            "; rm ",
            "; sudo ",
            "; shutdown ",
            "; reboot ",
            "; dd ",
            "; mkfs ",
            "; fdisk ",
            "> /dev/",
            "| bash",
            "| sh",
            "||",
            "&&",
        ];

        for pattern in dangerous_patterns.iter() {
            if lower_command.contains(pattern) {
                return Err(format!(
                    "Commande contient un pattern dangereux: '{}'",
                    pattern.trim()
                ));
            }
        }

        Ok(())
    }

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

// Fonction helper pour parser les variables d'environnement CSV
fn parse_csv_env_var(var_name: &str) -> Option<Vec<String>> {
    env::var(var_name).ok().map(|s| {
        s.split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()
    })
}

// Fonction helper pour lire un fichier s'il existe, avec une limite de taille optionnelle
fn load_file_if_exists(filepath: &str, max_size: Option<usize>) -> Option<String> {
    let path = Path::new(filepath);
    if !path.exists() {
        return None;
    }

    match fs::read_to_string(path) {
        Ok(content) => {
            match max_size {
                Some(limit) if content.len() > limit => {
                    // Tronquer à la limite, en essayant de couper sur un caractère UTF-8 valide
                    let truncated: String = content.chars().take(limit).collect();
                    Some(truncated)
                }
                _ => Some(content),
            }
        }
        Err(_e) => {
            // En mode debug, on pourrait logger l'erreur, mais on ignore silencieusement
            None
        }
    }
}

// ------------------ Main ------------------
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Variables d'environnement requises
    let api_key = env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY not found");

    // Variables d'environnement optionnelles
    let model = env::var("DEEPSEEK_AGENT_MODEL").ok();
    let mut system_prompt = env::var("DEEPSEEK_AGENT_SYSTEM_PROMPT").ok();

    // Chargement automatique des fichiers de contexte si non désactivé
    if std::env::var("DEEPSEEK_AGENT_SKIP_CONTEXT_FILES").is_err() {
        const MAX_CONTEXT_FILE_SIZE: usize = 10000; // caractères
        let debug = std::env::var("DEEPSEEK_AGENT_DEBUG").is_ok();
        let mut context_parts = Vec::new();

        // Charger AGENTS.md s'il existe
        if let Some(agents_content) = load_file_if_exists("AGENTS.md", Some(MAX_CONTEXT_FILE_SIZE))
        {
            if debug {
                println!(
                    "[Debug] Fichier AGENTS.md chargé ({} caractères)",
                    agents_content.len()
                );
            }
            context_parts.push(format!(
                "## Documentation AGENTS.md\n\n{}\n",
                agents_content
            ));
        } else if debug {
            println!("[Debug] Fichier AGENTS.md non trouvé ou erreur de lecture");
        }

        // Charger README.md s'il existe
        if let Some(readme_content) = load_file_if_exists("README.md", Some(MAX_CONTEXT_FILE_SIZE))
        {
            if debug {
                println!(
                    "[Debug] Fichier README.md chargé ({} caractères)",
                    readme_content.len()
                );
            }
            context_parts.push(format!(
                "## Documentation README.md\n\n{}\n",
                readme_content
            ));
        } else if debug {
            println!("[Debug] Fichier README.md non trouvé ou erreur de lecture");
        }

        if !context_parts.is_empty() {
            let context_str = context_parts.join("\n---\n");
            if debug {
                println!(
                    "[Debug] Contexte chargé à partir de {} fichier(s) ({} caractères totaux)",
                    context_parts.len(),
                    context_str.len()
                );
            }
            system_prompt = Some(match system_prompt {
                Some(existing) => format!("{}\n\n{}\n", existing, context_str),
                None => format!("Tu es un assistant qui peut exécuter des commandes shell. Pour cela, utilise l'outil 'sh' avec le paramètre 'command'.\n\n{}\n", context_str),
            });
            if debug {
                println!("[Debug] Prompt système enrichi avec la documentation");
            }
        } else if debug {
            println!("[Debug] Aucun fichier de contexte trouvé");
        }
    } else if std::env::var("DEEPSEEK_AGENT_DEBUG").is_ok() {
        println!("[Debug] Chargement des fichiers de contexte désactivé (DEEPSEEK_AGENT_SKIP_CONTEXT_FILES)");
    }

    // Listes CSV
    let whitelist = parse_csv_env_var("DEEPSEEK_AGENT_WHITELIST");
    let blacklist = parse_csv_env_var("DEEPSEEK_AGENT_BLACKLIST");

    // Limite d'historique (messages)
    let max_history_messages = env::var("DEEPSEEK_AGENT_MAX_HISTORY_MESSAGES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());

    // Limite de contexte (tokens)
    let max_context_tokens = env::var("DEEPSEEK_AGENT_MAX_CONTEXT_TOKENS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .or_else(|| {
            // Si non spécifié, calculer la valeur par défaut selon le modèle
            Some(default_max_context_tokens_for_model(&model))
        });

    // Gestion des retries
    let max_retries = env::var("DEEPSEEK_AGENT_MAX_RETRIES")
        .ok()
        .and_then(|s| s.parse::<u32>().ok());
    let retry_delay_ms = env::var("DEEPSEEK_AGENT_RETRY_DELAY_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let max_retry_delay_ms = env::var("DEEPSEEK_AGENT_MAX_RETRY_DELAY_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let shell_timeout_ms = env::var("DEEPSEEK_AGENT_SHELL_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());

    // Streaming des réponses (par défaut désactivé)
    let stream = env::var("DEEPSEEK_AGENT_STREAM")
        .ok()
        .map(|s| s.parse::<bool>().unwrap_or(false))
        .unwrap_or(false);

    // Log de configuration (debug)
    if std::env::var("DEEPSEEK_AGENT_DEBUG").is_ok() {
        println!("[Debug] Configuration chargée :");
        println!(
            "  Modèle: {:?}",
            model
                .as_ref()
                .unwrap_or(&"deepseek-chat (défaut)".to_string())
        );
        println!(
            "  Prompt système: {:?}",
            system_prompt
                .as_ref()
                .map(|_| "[présent]")
                .unwrap_or("défaut")
        );
        println!("  Liste blanche: {:?}", whitelist);
        println!("  Liste noire: {:?}", blacklist);
        println!("  Limite historique: {:?}", max_history_messages);
        println!("  Limite tokens: {:?}", max_context_tokens);
        let default_tokens = default_max_context_tokens_for_model(&model);
        println!(
            "  Limite tokens par défaut pour ce modèle: {}",
            default_tokens
        );
        println!("  Calibration tokens: activée");
        println!("  Max retries: {:?}", max_retries);
        println!("  Retry delay ms: {:?}", retry_delay_ms);
        println!("  Max retry delay ms: {:?}", max_retry_delay_ms);
        println!("  Shell timeout ms: {:?}", shell_timeout_ms);
        println!("  Streaming: {:?}", stream);
    }

    let debug = std::env::var("DEEPSEEK_AGENT_DEBUG").is_ok();

    let mut agent = Agent::new(
        api_key,
        model,
        system_prompt,
        whitelist,
        blacklist,
        max_history_messages,
        max_context_tokens,
        debug,
        max_retries,
        retry_delay_ms,
        max_retry_delay_ms,
        shell_timeout_ms,
        Some(stream),
    );

    agent.run().await
}

#[cfg(test)]
mod tests {
    use super::*;

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
