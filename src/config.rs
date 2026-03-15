use std::env;
use std::fs;
use std::path::Path;

// Constantes pour la gestion des erreurs et retries
pub const DEFAULT_MAX_RETRIES: u32 = 3;
pub const DEFAULT_RETRY_DELAY_MS: u64 = 1000;
pub const DEFAULT_MAX_RETRY_DELAY_MS: u64 = 30000;

// Fonction helper pour parser les variables d'environnement CSV
pub fn parse_csv_env_var(var_name: &str) -> Option<Vec<String>> {
    env::var(var_name).ok().map(|s| {
        s.split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()
    })
}

// Fonction helper pour lire un fichier s'il existe, avec une limite de taille optionnelle
pub fn load_file_if_exists(filepath: &str, max_size: Option<usize>) -> Option<String> {
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

// Configuration de l'agent
pub struct Config {
    pub api_key: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub whitelist: Option<Vec<String>>,
    pub blacklist: Option<Vec<String>>,
    pub max_history_messages: Option<usize>,
    pub max_context_tokens: Option<u32>,
    pub debug: bool,
    pub max_retries: Option<u32>,
    pub retry_delay_ms: Option<u64>,
    pub max_retry_delay_ms: Option<u64>,
    pub shell_timeout_ms: Option<u64>,
    pub stream: Option<bool>,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        // Variables d'environnement requises
        let api_key = env::var("DEEPSEEK_API_KEY")?;

        // Variables d'environnement optionnelles
        let model = env::var("DEEPSEEK_AGENT_MODEL").ok();
        let mut system_prompt = env::var("DEEPSEEK_AGENT_SYSTEM_PROMPT").ok();

        // Chargement automatique des fichiers de contexte si non désactivé
        if std::env::var("DEEPSEEK_AGENT_SKIP_CONTEXT_FILES").is_err() {
            const MAX_CONTEXT_FILE_SIZE: usize = 10000; // caractères
            let debug = std::env::var("DEEPSEEK_AGENT_DEBUG").is_ok();
            let mut context_parts = Vec::new();

            // Charger AGENTS.md s'il existe
            if let Some(agents_content) =
                load_file_if_exists("AGENTS.md", Some(MAX_CONTEXT_FILE_SIZE))
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
            if let Some(readme_content) =
                load_file_if_exists("README.md", Some(MAX_CONTEXT_FILE_SIZE))
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
                println!("[Debug] Fichier README.md non trouvu ou erreur de lecture");
            }

            // Charger CONTINUE.md s'il existe et le supprimer après lecture
            if let Some(continue_content) =
                load_file_if_exists("CONTINUE.md", Some(MAX_CONTEXT_FILE_SIZE))
            {
                if debug {
                    println!(
                        "[Debug] Fichier CONTINUE.md chargé ({} caractères)",
                        continue_content.len()
                    );
                }
                context_parts.push(format!(
                    "## Tâche en cours (CONTINUE.md)\n\n{}\n",
                    continue_content
                ));
                // Supprimer le fichier après lecture
                let _ = std::fs::remove_file("CONTINUE.md");
                if debug {
                    println!("[Debug] Fichier CONTINUE.md supprimé");
                }
            } else if debug {
                println!("[Debug] Fichier CONTINUE.md non trouvé ou erreur de lecture");
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
            .and_then(|s| s.parse::<u32>().ok());

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

        // Streaming des réponses (par défaut activé)
        let stream = env::var("DEEPSEEK_AGENT_STREAM")
            .ok()
            .map(|s| s.parse::<bool>().unwrap_or(true))
            .unwrap_or(true);

        let debug = std::env::var("DEEPSEEK_AGENT_DEBUG").is_ok();

        Ok(Self {
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
            stream: Some(stream),
        })
    }
}
