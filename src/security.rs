pub struct Security {
    whitelist: Option<Vec<String>>,
    blacklist: Option<Vec<String>>,
}

impl Security {
    pub fn new(whitelist: Option<Vec<String>>, blacklist: Option<Vec<String>>) -> Self {
        Self {
            whitelist,
            blacklist,
        }
    }

    // Valide une commande shell par rapport aux listes blanche/noire et règles de sécurité
    pub fn validate_command(&self, command: &str) -> Result<(), String> {
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
}