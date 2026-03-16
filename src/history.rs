use crate::api::Message;
use crate::token_management::{estimate_message_tokens, estimate_tokens};

pub struct HistoryManager {
    pub messages: Vec<Message>,
    pub total_tokens: u32,
    pub token_calibration_factor: f32,
    pub total_real_tokens_observed: u32,
    pub total_estimated_tokens: u32,
    pub max_history_messages: Option<usize>,
    pub max_context_tokens: Option<u32>,
    pub debug: bool,
}

impl HistoryManager {
    pub fn new(
        max_history_messages: Option<usize>,
        max_context_tokens: Option<u32>,
        debug: bool,
    ) -> Self {
        Self {
            messages: Vec::new(),
            total_tokens: 0,
            token_calibration_factor: 1.0,
            total_real_tokens_observed: 0,
            total_estimated_tokens: 0,
            max_history_messages,
            max_context_tokens,
            debug,
        }
    }

    // Met à jour l'estimation de tokens pour un message
    pub fn estimate_and_set_tokens(&mut self, message: &mut Message) -> u32 {
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
    pub fn add_message(&mut self, mut message: Message) {
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
    }

    // Calibre les estimations de tokens basées sur la réponse réelle de l'API
    pub fn calibrate_with_response(
        &mut self,
        request: &crate::api::ChatRequest,
        response: &crate::api::ChatResponse,
    ) {
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

    // Vérifie si la session doit être redémarrée (reste moins de 4000 tokens disponibles)
    pub fn should_restart_session(&self) -> bool {
        if let Some(max_tokens) = self.max_context_tokens {
            let remaining_tokens = max_tokens.saturating_sub(self.total_tokens);
            remaining_tokens <= 4000
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::*;

    #[test]
    fn test_history_manager_new() {
        let hm = HistoryManager::new(Some(10), Some(1000), false);
        assert_eq!(hm.messages.len(), 0);
        assert_eq!(hm.total_tokens, 0);
        assert_eq!(hm.token_calibration_factor, 1.0);
        assert_eq!(hm.max_history_messages, Some(10));
        assert_eq!(hm.max_context_tokens, Some(1000));
    }

    #[test]
    fn test_add_message() {
        let mut hm = HistoryManager::new(None, None, false);
        let message = Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
            tool_calls: None,
            tool_call_id: None,
            token_count: None,
        };
        hm.add_message(message);
        assert_eq!(hm.messages.len(), 1);
        assert!(hm.total_tokens > 0);
        // Le message devrait avoir un token_count estimé
        assert!(hm.messages[0].token_count.is_some());
    }

    #[test]
    fn test_add_multiple_messages() {
        let mut hm = HistoryManager::new(None, None, false);
        for i in 0..3 {
            let message = Message {
                role: "user".to_string(),
                content: format!("Message {}", i),
                tool_calls: None,
                tool_call_id: None,
                token_count: None,
            };
            hm.add_message(message);
        }
        assert_eq!(hm.messages.len(), 3);
        // Le total tokens devrait être la somme
        let total: u32 = hm.messages.iter().map(|m| m.token_count.unwrap_or(0)).sum();
        assert_eq!(hm.total_tokens, total);
    }

    #[test]
    fn test_estimate_and_set_tokens() {
        let mut hm = HistoryManager::new(None, None, false);
        let mut message = Message {
            role: "user".to_string(),
            content: "Hello world".to_string(),
            tool_calls: None,
            tool_call_id: None,
            token_count: None,
        };
        let tokens = hm.estimate_and_set_tokens(&mut message);
        assert!(tokens > 0);
        assert_eq!(message.token_count, Some(tokens));
        // Si on rappelle avec le même message (déjà un token_count), retourne la même valeur
        let tokens2 = hm.estimate_and_set_tokens(&mut message);
        assert_eq!(tokens2, tokens);
    }

    #[test]
    fn test_calibrate_with_response() {
        let mut hm = HistoryManager::new(None, None, false);
        // Ajouter un message pour avoir un historique
        let message = Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
            tool_calls: None,
            tool_call_id: None,
            token_count: None,
        };
        hm.add_message(message);
        
        // Créer une réponse fictive avec usage
        let response = ChatResponse {
            choices: vec![Choice {
                message: Message {
                    role: "assistant".to_string(),
                    content: "Hi there".to_string(),
                    tool_calls: None,
                    tool_call_id: None,
                    token_count: None,
                },
            }],
            usage: Usage {
                prompt_tokens: 100,
                completion_tokens: 20,
                total_tokens: 120,
                prompt_cache_hit_tokens: Some(50),
                prompt_cache_miss_tokens: Some(50),
            },
        };
        
        // Créer une requête fictive avec outils
        let request = ChatRequest {
            model: "deepseek-chat".to_string(),
            messages: hm.messages.clone(),
            tools: vec![Tool {
                tool_type: "function".to_string(),
                function: ToolFunction {
                    name: "sh".to_string(),
                    description: "Execute shell command".to_string(),
                    parameters: serde_json::json!({}),
                },
            }],
            tool_choice: "auto".to_string(),
            stream: false,
        };
        
        // Calibrer
        hm.calibrate_with_response(&request, &response);
        
        // Vérifier que les statistiques ont été mises à jour
        assert_eq!(hm.total_real_tokens_observed, 100);
        assert!(hm.total_estimated_tokens > 0);
        // Le facteur de calibration peut avoir changé s'il y avait une erreur significative
        assert!(hm.token_calibration_factor >= 0.5 && hm.token_calibration_factor <= 2.0);
    }

    #[test]
    fn test_should_restart_session() {
        // Cas sans limite de tokens
        let hm = HistoryManager::new(None, None, false);
        assert!(!hm.should_restart_session());
        
        // Cas avec limite mais assez de tokens restants
        let mut hm = HistoryManager::new(None, Some(10000), false);
        // Simuler un total_tokens de 1000
        hm.total_tokens = 1000;
        assert!(!hm.should_restart_session()); // 9000 restants > 4000
        
        // Cas avec peu de tokens restants
        hm.total_tokens = 7000;
        assert!(hm.should_restart_session()); // 3000 restants <= 4000
        
        // Cas exactement au seuil
        hm.total_tokens = 6000;
        assert!(hm.should_restart_session()); // 4000 restants <= 4000
        
        // Cas dépassé
        hm.total_tokens = 11000;
        assert!(hm.should_restart_session()); // -1000 restants <= 4000
    }
}
