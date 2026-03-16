//! Interface utilisateur améliorée avec couleurs.

use colored::*;
use std::env;
use std::io::{self, IsTerminal};

/// Vérifie si les couleurs doivent être activées.
/// Priorité : variable DEEPSEEK_AGENT_NO_COLOR=1 > DEEPSEEK_AGENT_COLOR=1 > détection automatique (tty)
pub fn colors_enabled() -> bool {
    // Vérifier la variable d'environnement DEEPSEEK_AGENT_NO_COLOR
    if env::var("DEEPSEEK_AGENT_NO_COLOR").is_ok() {
        return false;
    }
    // Vérifier la variable DEEPSEEK_AGENT_COLOR
    if env::var("DEEPSEEK_AGENT_COLOR").is_ok() {
        return true;
    }
    // Détection automatique : stdout est un terminal
    io::stdout().is_terminal()
}

/// Initialise la gestion des couleurs (appeler au démarrage)
pub fn init_colors() {
    if !colors_enabled() {
        colored::control::set_override(false);
    } else {
        colored::control::set_override(true);
    }
}

/// Styles pour différents types de messages
pub struct Theme {
    pub user_prompt: String,
    pub assistant_prefix: String,
    pub shell_command: String,
    pub shell_output: String,
    pub error: String,
    pub warning: String,
    pub debug: String,
    pub info: String,
    pub token_count: String,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            user_prompt: ">>".green().bold().to_string(),
            assistant_prefix: "Agent:".blue().bold().to_string(),
            shell_command: "[Shell]".cyan().bold().to_string(),
            shell_output: "".normal().to_string(),
            error: "[Erreur]".red().bold().to_string(),
            warning: "[Attention]".yellow().bold().to_string(),
            debug: "[Debug]".yellow().dimmed().to_string(),
            info: "[Info]".cyan().dimmed().to_string(),
            token_count: "[Tokens]".magenta().dimmed().to_string(),
        }
    }
}

/// Formateur de messages avec thème
pub struct MessageFormatter {
    theme: Theme,
    colors_enabled: bool,
}

impl MessageFormatter {
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
            colors_enabled: colors_enabled(),
        }
    }

    pub fn user_prompt(&self) -> String {
        if self.colors_enabled {
            self.theme.user_prompt.clone()
        } else {
            ">>".to_string()
        }
    }

    pub fn assistant_prefix(&self) -> String {
        if self.colors_enabled {
            self.theme.assistant_prefix.clone()
        } else {
            "Agent:".to_string()
        }
    }

    pub fn shell_command(&self) -> String {
        if self.colors_enabled {
            self.theme.shell_command.clone()
        } else {
            "[Shell]".to_string()
        }
    }

    pub fn error(&self) -> String {
        if self.colors_enabled {
            self.theme.error.clone()
        } else {
            "[Erreur]".to_string()
        }
    }

    pub fn warning(&self) -> String {
        if self.colors_enabled {
            self.theme.warning.clone()
        } else {
            "[Attention]".to_string()
        }
    }

    pub fn debug(&self) -> String {
        if self.colors_enabled {
            self.theme.debug.clone()
        } else {
            "[Debug]".to_string()
        }
    }

    pub fn info(&self) -> String {
        if self.colors_enabled {
            self.theme.info.clone()
        } else {
            "[Info]".to_string()
        }
    }

    pub fn token_count(&self) -> String {
        if self.colors_enabled {
            self.theme.token_count.clone()
        } else {
            "[Tokens]".to_string()
        }
    }

    /// Formate un message utilisateur (sans prompt)
    pub fn user_message(&self, text: &str) -> String {
        if self.colors_enabled {
            text.green().to_string()
        } else {
            text.to_string()
        }
    }

    /// Formate un message assistant
    pub fn assistant_message(&self, text: &str) -> String {
        if self.colors_enabled {
            text.blue().to_string()
        } else {
            text.to_string()
        }
    }

    /// Formate une commande shell
    pub fn shell_command_message(&self, command: &str) -> String {
        if self.colors_enabled {
            format!("{} {}", self.shell_command(), command.cyan())
        } else {
            format!("[Shell] {}", command)
        }
    }

    /// Formate une sortie shell
    pub fn shell_output(&self, output: &str) -> String {
        if self.colors_enabled {
            output.dimmed().to_string()
        } else {
            output.to_string()
        }
    }

    /// Formate un message d'erreur
    pub fn error_message(&self, text: &str) -> String {
        if self.colors_enabled {
            format!("{} {}", self.error(), text.red())
        } else {
            format!("[Erreur] {}", text)
        }
    }

    /// Formate un message d'avertissement
    pub fn warning_message(&self, text: &str) -> String {
        if self.colors_enabled {
            format!("{} {}", self.warning(), text.yellow())
        } else {
            format!("[Attention] {}", text)
        }
    }

    /// Formate un message de debug
    pub fn debug_message(&self, text: &str) -> String {
        if self.colors_enabled {
            format!("{} {}", self.debug(), text.yellow())
        } else {
            format!("[Debug] {}", text)
        }
    }

    /// Formate un message d'info
    pub fn info_message(&self, text: &str) -> String {
        if self.colors_enabled {
            format!("{} {}", self.info(), text.cyan())
        } else {
            format!("[Info] {}", text)
        }
    }

    /// Formate un compte de tokens
    pub fn token_message(&self, text: &str) -> String {
        if self.colors_enabled {
            format!("{} {}", self.token_count(), text.magenta())
        } else {
            format!("[Tokens] {}", text)
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_colors_enabled_without_env() {
        // Ce test dépend de l'environnement, difficile à tester unitairement.
        // On vérifie juste que la fonction ne panique pas
        let _ = colors_enabled();
    }
    
    #[test]
    fn test_message_formatter_new() {
        let formatter = MessageFormatter::new();
        // Vérifier que les méthodes ne paniquent pas
        let _ = formatter.user_prompt();
        let _ = formatter.assistant_prefix();
        let _ = formatter.shell_command();
    }
}