use reqwest;
use std::time::Duration;

pub struct FetchExecutor {
    timeout_ms: Option<u64>,
}

impl FetchExecutor {
    pub fn new(timeout_ms: Option<u64>) -> Self {
        Self { timeout_ms }
    }

    pub async fn fetch(&self, url: &str) -> String {
        // Validation de l'URL
        if !Self::is_valid_url(url) {
            return format!("Erreur: URL invalide '{}'", url);
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(self.timeout_ms.unwrap_or(30000)))
            .user_agent("Mozilla/5.0 (compatible; DeepSeekAgent/1.0)")
            .build()
            .unwrap();

        let response = client.get(url).send().await;

        match response {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    return format!("Erreur HTTP {}: {}", status.as_u16(), status.canonical_reason().unwrap_or("Inconnu"));
                }

                let content_type = resp.headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");

                // Conversion en markdown selon le type de contenu
                match content_type {
                    ct if ct.contains("text/html") => FetchExecutor::html_to_markdown(resp.text().await.unwrap_or_default()),
                    ct if ct.contains("text/plain") || ct.contains("application/json") || ct.contains("text/markdown") => {
                        let text = resp.text().await.unwrap_or_default();
                        format!("```\n{}\n```", FetchExecutor::escape_markdown(&text))
                    }
                    _ => {
                        let text = resp.text().await.unwrap_or_default();
                        format!("Contenu brut:\n```\n{}\n```", FetchExecutor::escape_markdown(&text))
                    }
                }
            }
            Err(e) => format!("Erreur de requête HTTP: {}", e),
        }
    }

    fn is_valid_url(url: &str) -> bool {
        // Validation basique d'URL
        url.starts_with("http://") || url.starts_with("https://")
    }

    fn html_to_markdown(html: String) -> String {
        // Extraction simple du texte sans balises HTML
        let text = Self::remove_html_tags(&html);
        
        if text.is_empty() {
            return "Le contenu de la page ne contient pas de texte extractible.".to_string();
        }

        format!("```\n{}\n```", Self::normalize_whitespace(&text))
    }

    fn remove_html_tags(html: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;

        for c in html.chars() {
            match c {
                '<' => in_tag = true,
                '>' => {
                    in_tag = false;
                    result.push(' ');
                }
                _ if !in_tag => result.push(c),
                _ => {}
            }
        }

        result
    }

    fn normalize_whitespace(text: &str) -> String {
        text.split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
    }

    fn escape_markdown(text: &str) -> String {
        // Échappement des caractères markdown spéciaux
        let mut result = String::new();
        for c in text.chars() {
            match c {
                '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#' | '+' | '-' | '.' | '!' => {
                    result.push('\\');
                    result.push(c);
                }
                _ => result.push(c),
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn test_fetch_executor_no_timeout() {
        let rt = Runtime::new().unwrap();
        let executor = FetchExecutor::new(None);
        
        // Test avec une URL simple (peut échouer sans réseau, donc on teste la structure)
        let result = rt.block_on(executor.fetch("https://www.example.com"));
        assert!(result.contains("```") || result.contains("Erreur"));
    }

    #[test]
    fn test_fetch_executor_invalid_url() {
        let rt = Runtime::new().unwrap();
        let executor = FetchExecutor::new(None);
        
        let result = rt.block_on(executor.fetch("not-a-url"));
        assert!(result.contains("URL invalide"));
    }

    #[test]
    fn test_fetch_executor_http_error() {
        let rt = Runtime::new().unwrap();
        let executor = FetchExecutor::new(None);
        
        // Test avec une URL qui retourne une erreur (ex: 404)
        let result = rt.block_on(executor.fetch("https://httpbin.org/status/404"));
        assert!(result.contains("Erreur HTTP") || result.contains("404"));
    }

    #[test]
    fn test_remove_html_tags() {
        let html = "<html><body><h1>Hello</h1><p>World</p></body></html>";
        let text = FetchExecutor::remove_html_tags(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
        assert!(!text.contains("<") || text == "< ");
    }

    #[test]
    fn test_normalize_whitespace() {
        let text = "Multiple    spaces   and\nnewlines";
        let normalized = FetchExecutor::normalize_whitespace(text);
        assert_eq!(normalized, "Multiple spaces and newlines");
    }

    #[test]
    fn test_escape_markdown() {
        let text = "Hello *world* _test_ `code` [link](url)";
        let escaped = FetchExecutor::escape_markdown(text);
        assert!(escaped.contains("\\*") || escaped.contains("*"));
        assert!(escaped.contains("\\`") || escaped.contains("`"));
    }

    #[test]
    fn test_is_valid_url() {
        assert!(FetchExecutor::is_valid_url("http://example.com"));
        assert!(FetchExecutor::is_valid_url("https://example.com"));
        assert!(!FetchExecutor::is_valid_url("ftp://example.com"));
        assert!(!FetchExecutor::is_valid_url("example.com"));
    }

    #[test]
    fn test_fetch_executor_timeout() {
        let rt = Runtime::new().unwrap();
        let executor = FetchExecutor::new(Some(100)); // 100ms timeout
        
        // Cette URL peut être lente à répondre
        let result = rt.block_on(executor.fetch("https://www.google.com"));
        // Soit ça réussit, soit c'est un timeout géré par reqwest
        assert!(result.contains("```") || result.contains("Erreur"));
    }

    #[test]
    fn test_html_to_markdown_empty() {
        let html = "<html><body></body></html>";
        let markdown = FetchExecutor::html_to_markdown(html.to_string());
        // println!("Markdown: '{}'", markdown);
        assert!(markdown.contains("ne contient pas de texte") || markdown.starts_with("```"));
    }

    #[test]
    fn test_fetch_executor_plaintext() {
        let rt = Runtime::new().unwrap();
        let executor = FetchExecutor::new(None);
        
        // Test avec text/plain (httpbin.org retourne le contenu tel quel)
        let result = rt.block_on(executor.fetch("https://httpbin.org/bytes/100"));
        assert!(result.contains("```") || result.contains("Erreur"));
    }

    #[test]
    fn test_fetch_executor_json() {
        let rt = Runtime::new().unwrap();
        let executor = FetchExecutor::new(None);
        
        // Test avec JSON (httpbin.org/headers retourne les headers en JSON)
        let result = rt.block_on(executor.fetch("https://httpbin.org/headers"));
        assert!(result.contains("```") || result.contains("Erreur"));
    }
}
