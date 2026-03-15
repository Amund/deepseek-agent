use tokio::process::Command;
use tokio::time::{timeout, Duration};

pub struct ShellExecutor {
    timeout_ms: Option<u64>,
}

impl ShellExecutor {
    pub fn new(timeout_ms: Option<u64>) -> Self {
        Self { timeout_ms }
    }

    pub async fn exec(&self, command: &str) -> String {
        // Exécution avec timeout optionnel
        let cmd_future = Command::new("sh").arg("-c").arg(command).output();

        let output = if let Some(timeout_ms) = self.timeout_ms {
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
}
