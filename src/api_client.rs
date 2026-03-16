use crate::api::*;
use crate::streaming::StreamProcessor;
use reqwest::Client;
use tokio::time::{sleep, Duration};

pub struct ApiClient {
    client: Client,
    api_key: String,
    #[allow(dead_code)]
    model: String,
    base_url: String,
    stream: bool,
    debug: bool,
    max_retries: u32,
    retry_delay_ms: u64,
    max_retry_delay_ms: u64,
}

impl ApiClient {
    pub fn new(
        api_key: String,
        model: Option<String>,
        stream: bool,
        debug: bool,
        max_retries: u32,
        retry_delay_ms: u64,
        max_retry_delay_ms: u64,
        base_url: Option<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| "deepseek-chat".to_string()),
            base_url: base_url.unwrap_or_else(|| "https://api.deepseek.com".to_string()),
            stream,
            debug,
            max_retries,
            retry_delay_ms,
            max_retry_delay_ms,
        }
    }

    // Calcule le délai de retry pour une tentative donnée
    fn calculate_retry_delay(&self, attempt: u32) -> u64 {
        let multiplier = 2u64.checked_pow(attempt).unwrap_or(u64::MAX);
        let delay = self.retry_delay_ms.checked_mul(multiplier).unwrap_or(u64::MAX);
        std::cmp::min(delay, self.max_retry_delay_ms)
    }

    // Effectue un appel à l'API DeepSeek avec gestion des retries
    pub async fn call(
        &self,
        request: &ChatRequest,
    ) -> Result<ChatResponse, Box<dyn std::error::Error>> {
        let mut last_error: Option<Box<dyn std::error::Error>> = None;

        for attempt in 0..=self.max_retries {
            match self
                .client
                .post(format!("{}/v1/chat/completions", self.base_url.trim_end_matches('/')))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&request)
                .send()
                .await
            {
                Ok(resp) => {
                    // Vérifier le statut HTTP
                    if resp.status().is_success() {
                        if self.stream {
                            if self.debug {
                                println!("[Debug] Using streaming mode for API call");
                            }
                            let stream_processor = StreamProcessor::new(self.debug);
                            match stream_processor.process_response(resp).await {
                                Ok(response) => {
                                    return Ok(response);
                                }
                                Err(e) => {
                                    last_error = Some(e.into());
                                    if self.debug {
                                        println!(
                                            "[Debug] Error processing streaming response on attempt {}: {}",
                                            attempt,
                                            last_error.as_ref().unwrap()
                                        );
                                    }
                                }
                            }
                        } else {
                            // Mode non-streaming
                            match resp.text().await {
                                Ok(text) => {
                                    if self.debug && attempt == 0 {
                                        println!("[Debug] API response text (first {} chars): {}...", text.len().min(200), text.chars().take(200).collect::<String>());
                                    }
                                    match serde_json::from_str::<ChatResponse>(&text) {
                                        Ok(response) => {
                                            return Ok(response);
                                        }
                                        Err(e) => {
                                            last_error = Some(format!("Error parsing JSON response: {}. Text: {}...", e, text.chars().take(200).collect::<String>()).into());
                                            if self.debug {
                                                println!(
                                                    "[Debug] Error parsing response on attempt {}: {}",
                                                    attempt,
                                                    last_error.as_ref().unwrap()
                                                );
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    last_error = Some(format!("Error reading response body: {}", e).into());
                                    if self.debug {
                                        println!(
                                            "[Debug] Error reading response body on attempt {}: {}",
                                            attempt,
                                            last_error.as_ref().unwrap()
                                        );
                                    }
                                }
                            }
                        }
                    } else {
                        // Erreur HTTP (429, 500, etc.)
                        let status = resp.status();
                        let text = resp.text().await.unwrap_or_default();
                        last_error = Some(format!("HTTP {}: {}", status, text).into());
                        if self.debug {
                            println!(
                                "[Debug] HTTP error on attempt {}: {}",
                                attempt,
                                last_error.as_ref().unwrap()
                            );
                        }

                        // Si c'est une erreur client (4xx) sauf 429 (rate limit), on ne retry pas
                        if status.is_client_error() && status != 429 {
                            break;
                        }
                        // Pour les autres erreurs (429, 5xx), on retry
                    }
                }
                Err(e) => {
                    last_error = Some(e.into());
                    if self.debug {
                        println!(
                            "[Debug] Network error on attempt {}: {}",
                            attempt,
                            last_error.as_ref().unwrap()
                        );
                    }
                }
            }

            // Si ce n'est pas la dernière tentative, attendre avant de retry
            if attempt < self.max_retries {
                let delay_ms = self.calculate_retry_delay(attempt);
                if self.debug {
                    println!("[Debug] Retrying in {} ms...", delay_ms);
                }
                sleep(Duration::from_millis(delay_ms)).await;
            }
        }

        // Si on arrive ici, toutes les tentatives ont échoué
        Err(last_error.unwrap_or_else(|| "Unknown error".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_client_new() {
        let client = ApiClient::new(
            "test_key".to_string(),
            Some("deepseek-chat".to_string()),
            false,
            false,
            3,
            100,
            1000,
            None,
        );
        assert_eq!(client.api_key, "test_key");
        assert_eq!(client.model, "deepseek-chat");
        assert!(!client.stream);
        assert!(!client.debug);
        assert_eq!(client.max_retries, 3);
        assert_eq!(client.retry_delay_ms, 100);
        assert_eq!(client.max_retry_delay_ms, 1000);
    }

    #[test]
    fn test_api_client_new_default_model() {
        let client = ApiClient::new(
            "test_key".to_string(),
            None,
            false,
            false,
            3,
            100,
            1000,
            None,
        );
        assert_eq!(client.model, "deepseek-chat");
    }

    #[test]
    fn test_calculate_retry_delay() {
        let client = ApiClient::new(
            "test_key".to_string(),
            None,
            false,
            false,
            3,
            100,
            1000,
            None,
        );
        // Tentative 0: 100 * 2^0 = 100
        assert_eq!(client.calculate_retry_delay(0), 100);
        // Tentative 1: 100 * 2^1 = 200
        assert_eq!(client.calculate_retry_delay(1), 200);
        // Tentative 2: 100 * 2^2 = 400
        assert_eq!(client.calculate_retry_delay(2), 400);
        // Tentative 3: 100 * 2^3 = 800
        assert_eq!(client.calculate_retry_delay(3), 800);
        // Tentative 4: 100 * 2^4 = 1600, mais limité à max_retry_delay_ms = 1000
        assert_eq!(client.calculate_retry_delay(4), 1000);
        // Tentative 10: toujours limité à 1000
        assert_eq!(client.calculate_retry_delay(10), 1000);
    }

    #[test]
    fn test_calculate_retry_delay_no_overflow() {
        // Test avec des valeurs qui pourraient causer un overflow
        let client = ApiClient::new(
            "test_key".to_string(),
            None,
            false,
            false,
            3,
            u64::MAX,
            u64::MAX,
            None,
        );
        // Tentative 0: u64::MAX * 1 = u64::MAX
        assert_eq!(client.calculate_retry_delay(0), u64::MAX);
        // Tentative 1: overflow potentiel dans 2u64.pow(attempt), mais multiplication avec u64::MAX peut dépasser.
        // Cependant, std::cmp::min avec u64::MAX limitera.
        // On vérifie que la fonction ne panique pas.
        let _ = client.calculate_retry_delay(1);
    }

    // Note: les tests pour la méthode call nécessitent des mocks HTTP.
    // On peut les ajouter plus tard avec un crate comme mockito.
}
