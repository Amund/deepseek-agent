// Utilisation de la librairie deepseek_agent
use deepseek_agent::{Agent, Config, RestartSessionError, default_max_context_tokens_for_model};

// ------------------ Main ------------------
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Charger la configuration depuis l'environnement
    let config = Config::from_env()?;

    // Initialiser le gestionnaire d'interruption (Ctrl+C)
    deepseek_agent::interrupt::init_interrupt_handler();

    // Calculer la limite de tokens par défaut si non spécifiée
    let max_context_tokens = config
        .max_context_tokens
        .or_else(|| Some(default_max_context_tokens_for_model(&config.model)));

    // Boucle de redémarrage de session
    let mut restart_count = 0;
    const MAX_RESTARTS: u32 = 10;

    loop {
        // Créer l'agent
        let mut agent = Agent::new(
            config.api_key.clone(),
            config.model.clone(),
            config.system_prompt.clone(),

            config.max_history_messages,
            max_context_tokens,
            config.debug,
            config.max_retries,
            config.retry_delay_ms,
            config.max_retry_delay_ms,
            config.shell_timeout_ms,
            config.stream,
            None,
        );

        // Exécuter l'agent
        match agent.run().await {
            Ok(()) => {
                // Session terminée normalement (utilisateur a tapé 'quit')
                break;
            }
            Err(e) => {
                // Vérifier si c'est une erreur de redémarrage de session
                if let Some(_restart_err) = e.downcast_ref::<RestartSessionError>() {
                    restart_count += 1;
                    if restart_count > MAX_RESTARTS {
                        eprintln!("Trop de redémarrages de session ({}). Arrêt.", MAX_RESTARTS);
                        break;
                    }
                    println!(
                        "🔁 Redémarrage de la session ({}). Création du fichier CONTINUE.md...",
                        restart_count
                    );
                    // Le fichier CONTINUE.md a déjà été créé par l'agent
                    // On continue la boucle pour créer un nouvel agent
                } else {
                    // Autre erreur, on propage
                    return Err(e);
                }
            }
        }
    }

    Ok(())
}

