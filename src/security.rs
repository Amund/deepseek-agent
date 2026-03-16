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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_no_lists() {
        let security = Security::new(None, None);
        assert!(security.validate_command("ls -la").is_ok());
        assert!(security.validate_command("echo hello").is_ok());
        // Les commandes dangereuses sont toujours détectées par les patterns
        assert!(security.validate_command("ls; rm -rf /").is_err());
    }

    #[test]
    fn test_security_whitelist() {
        let whitelist = Some(vec!["ls".to_string(), "echo".to_string()]);
        let security = Security::new(whitelist, None);
        assert!(security.validate_command("ls -la").is_ok());
        assert!(security.validate_command("echo hello").is_ok());
        assert!(security.validate_command("cat file.txt").is_err());
        assert!(security.validate_command("rm file.txt").is_err());
    }

    #[test]
    fn test_security_blacklist() {
        let blacklist = Some(vec!["rm".to_string(), "dd".to_string()]);
        let security = Security::new(None, blacklist);
        assert!(security.validate_command("ls -la").is_ok());
        assert!(security.validate_command("echo hello").is_ok());
        assert!(security.validate_command("rm file.txt").is_err());
        assert!(security.validate_command("dd if=/dev/zero").is_err());
        // Vérification sur tous les tokens (même si rm est le second token)
        assert!(security.validate_command("ls; rm file.txt").is_err());
        // Les options commençant par - ne sont pas bloquées
        assert!(security.validate_command("ls -rm").is_ok());
    }

    #[test]
    fn test_security_blacklist_priority() {
        // Blacklist prioritaire sur whitelist
        let whitelist = Some(vec!["rm".to_string()]);
        let blacklist = Some(vec!["rm".to_string()]);
        let security = Security::new(whitelist, blacklist);
        assert!(security.validate_command("rm file.txt").is_err());
    }

    #[test]
    fn test_security_dangerous_patterns() {
        let security = Security::new(None, None);
        // Patterns dangereux
        assert!(security.validate_command("ls; rm -rf /").is_err());
        assert!(security.validate_command("ls; sudo apt update").is_err());
        assert!(security.validate_command("ls > /dev/null").is_err());
        assert!(security.validate_command("echo test | bash").is_err());
        assert!(security.validate_command("echo test | sh").is_err());
        assert!(security.validate_command("ls && rm -rf /").is_err());
        assert!(security.validate_command("ls || echo fail").is_err());
        // Commandes normales
        assert!(security.validate_command("ls -la").is_ok());
        assert!(security.validate_command("echo hello world").is_ok());
        // Patterns avec variations de casse
        assert!(security.validate_command("ls; SUDO apt update").is_err());
    }

    #[test]
    fn test_security_empty_command() {
        let security = Security::new(None, None);
        assert!(security.validate_command("").is_ok());
        let security = Security::new(Some(vec!["ls".to_string()]), None);
        assert!(security.validate_command("").is_err()); // cmd_name vide n'est pas dans la whitelist
    }
}
