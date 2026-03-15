use crate::api::*;
use crate::streaming::StreamProcessor;
use reqwest::Client;
use tokio::time::{sleep, Duration};

pub struct ApiClient {
    client: Client,
    api_key: String,
    model: String,
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
    ) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| "deepseek-chat".to_string()),
            stream,
            debug,
            max_retries,
            retry_delay_ms,
            max_retry_delay_ms,
        }
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
                .post("https://api.deepseek.com/v1/chat/completions")
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
                            match resp.json::<ChatResponse>().await {
                                Ok(response) => {
                                    return Ok(response);
                                }
                                Err(e) => {
                                    last_error = Some(e.into());
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
                // Backoff exponentiel avec délai maximum
                let delay_ms = std::cmp::min(
                    self.retry_delay_ms * 2u64.pow(attempt),
                    self.max_retry_delay_ms,
                );
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
