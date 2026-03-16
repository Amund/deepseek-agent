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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn test_shell_executor_no_timeout() {
        let rt = Runtime::new().unwrap();
        let executor = ShellExecutor::new(None);
        let output = rt.block_on(executor.exec("echo hello"));
        assert!(output.contains("hello"));
    }

    #[test]
    fn test_shell_executor_with_timeout() {
        let rt = Runtime::new().unwrap();
        let executor = ShellExecutor::new(Some(5000)); // 5 secondes
        let output = rt.block_on(executor.exec("echo test"));
        assert!(output.contains("test"));
    }

    #[test]
    fn test_shell_executor_timeout_expired() {
        let rt = Runtime::new().unwrap();
        let executor = ShellExecutor::new(Some(100)); // 100ms
        let output = rt.block_on(executor.exec("sleep 1")); // dort 1 seconde
        assert!(output.contains("Timeout"));
    }

    #[test]
    fn test_shell_executor_error() {
        let rt = Runtime::new().unwrap();
        let executor = ShellExecutor::new(None);
        let output = rt.block_on(executor.exec("non_existent_command"));
        assert!(output.contains("Erreur d'exécution") || output.contains("STDERR"));
    }

    #[test]
    fn test_shell_executor_stderr() {
        let rt = Runtime::new().unwrap();
        let executor = ShellExecutor::new(None);
        let output = rt.block_on(executor.exec("ls /nonexistent"));
        // Cette commande génère une erreur (fichier inexistant)
        assert!(output.contains("STDERR") || output.contains("No such file"));
    }
}
