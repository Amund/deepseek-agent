use crate::api::Message;

// Fonction pour estimer le nombre de tokens dans un texte
// Estimation basée sur la longueur (approximation)
// En pratique, utiliser les retours de l'API pour plus de précision
pub fn estimate_tokens(text: &str) -> u32 {
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

pub fn estimate_message_tokens(message: &Message) -> u32 {
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
pub fn default_max_context_tokens_for_model(model: &Option<String>) -> u32 {
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