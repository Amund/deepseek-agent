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
