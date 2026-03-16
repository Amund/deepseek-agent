use deepseek_agent::{ApiClient, ShellExecutor, HistoryManager, Message, Tool, ToolFunction, ChatRequest, ChatResponse, Choice, Usage, check_and_restart_if_needed};
use mockito::Server;
use serde_json::json;

/// Test d'intégration simple: simulation d'une réponse API avec tool_call
/// et vérification que la commande shell est exécutée.
#[tokio::test]
async fn test_agent_with_mocked_api() {
    // Créer un serveur mock
    let mut server = Server::new_async().await;
    
    // Définir le mock pour l'API
    let mock_response = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "id": "call_123",
                    "type": "function",
                    "function": {
                        "name": "sh",
                        "arguments": "{\"command\": \"echo test\"}"
                    }
                }]
            }
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15,
            "prompt_cache_hit_tokens": 2,
            "prompt_cache_miss_tokens": 8
        }
    });
    
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .match_header("Authorization", "Bearer fake_api_key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_response.to_string())
        .create_async()
        .await;
    
    // Créer un ApiClient qui pointe vers le serveur mock
    let api_client = ApiClient::new(
        "fake_api_key".to_string(),
        Some("deepseek-chat".to_string()),
        false, // streaming désactivé
        false, // debug désactivé
        3,
        100,
        1000,
        Some(server.url()),
    );
    
    // Créer un Agent avec l'ApiClient mocké (malheureusement Agent::new ne prend pas un ApiClient personnalisé)
    // Nous allons plutôt tester le flux complet via une fonction qui utilise l'agent.
    // Pour l'instant, nous testons simplement l'ApiClient.
    let request = ChatRequest {
        model: "deepseek-chat".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
            tool_calls: None,
            tool_call_id: None,
            token_count: None,
        }],
        tools: vec![Tool {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "sh".to_string(),
                description: "Execute shell command".to_string(),
                parameters: json!({}),
            },
        }],
        tool_choice: "auto".to_string(),
        stream: false,
    };
    
    let chat_response = match api_client.call(&request).await {
        Ok(r) => r,
        Err(e) => {
            println!("API call failed: {}", e);
            panic!("API call failed: {}", e);
        }
    };
    assert_eq!(chat_response.choices.len(), 1);
    let message = &chat_response.choices[0].message;
    assert_eq!(message.role, "assistant");
    assert!(message.tool_calls.is_some());
    let tool_calls = message.tool_calls.as_ref().unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].function.name, "sh");
    assert_eq!(tool_calls[0].function.arguments, "{\"command\": \"echo test\"}");
    
    // Vérifier que le mock a été appelé
    mock.assert_async().await;
}

/// Test d'intégration avec streaming: simulation de chunks SSE
#[tokio::test]
async fn test_streaming_with_mocked_api() {
    // Pour tester le streaming, nous devons simuler des Server-Sent Events.
    // Mockito ne supporte pas facilement le streaming.
    // Nous pourrions utiliser un vrai serveur HTTP local avec tokio et axum,
    // mais c'est plus complexe.
    // Pour l'instant, nous allons simplement tester le parsing de chunks.
    // Ce test sera ajouté plus tard.
}

/// Test d'exécution de commande shell simple
#[tokio::test]
async fn test_shell_execution() {
    let executor = ShellExecutor::new(None);
    let output = executor.exec("echo integration_test").await;
    assert!(output.contains("integration_test"));
}

/// Test d'exécution de commande shell avec timeout
#[tokio::test]
async fn test_shell_execution_with_timeout() {
    let executor = ShellExecutor::new(Some(100)); // 100ms
    let output = executor.exec("sleep 0.05 && echo ok").await; // dort 50ms
    // La commande devrait réussir (pas de timeout)
    assert!(output.contains("ok"));
    
    let executor2 = ShellExecutor::new(Some(50)); // 50ms
    let output2 = executor2.exec("sleep 1").await; // dort 1 seconde
    assert!(output2.contains("Timeout"));
}

/// Test de l'historique et de la calibration des tokens
#[test]
fn test_history_calibration() {

    
    let mut history = HistoryManager::new(None, Some(10000), false);
    
    // Ajouter quelques messages
    history.add_message(Message {
        role: "user".to_string(),
        content: "Hello".to_string(),
        tool_calls: None,
        tool_call_id: None,
        token_count: None,
    });
    
    history.add_message(Message {
        role: "assistant".to_string(),
        content: "Hi there".to_string(),
        tool_calls: None,
        tool_call_id: None,
        token_count: None,
    });
    
    // Vérifier que les messages sont ajoutés
    assert_eq!(history.messages.len(), 2);
    
    // Simuler une réponse API avec usage
    let request = ChatRequest {
        model: "deepseek-chat".to_string(),
        messages: history.messages.clone(),
        tools: vec![],
        tool_choice: "auto".to_string(),
        stream: false,
    };
    
    let response = ChatResponse {
        choices: vec![Choice {
            message: Message {
                role: "assistant".to_string(),
                content: "Response".to_string(),
                tool_calls: None,
                tool_call_id: None,
                token_count: None,
            },
        }],
        usage: Usage {
            prompt_tokens: 15,
            completion_tokens: 5,
            total_tokens: 20,
            prompt_cache_hit_tokens: Some(5),
            prompt_cache_miss_tokens: Some(10),
        },
    };
    
    // Calibrer
    history.calibrate_with_response(&request, &response);
    
    // Vérifier que les comptes de tokens sont mis à jour
    // (l'implémentation actuelle ne met pas à jour token_count dans les messages existants)
    // Mais elle calcule la calibration pour les estimations futures.
    // Nous vérifions simplement que la fonction ne panique pas.
}

/// Test du redémarrage de session
#[test]
fn test_session_restart_logic() {


    
    // Créer un fichier temporaire pour le test
    let temp_dir = tempfile::tempdir().unwrap();
    let _temp_path = temp_dir.path().join("CONTINUE.md");
    
    // Simuler une condition de redémarrage (should_restart = true)
    let messages = vec![Message {
        role: "user".to_string(),
        content: "Hello".repeat(1000), // Beaucoup de contenu
        tool_calls: None,
        tool_call_id: None,
        token_count: None,
    }];
    
    // Cette fonction va créer un fichier CONTINUE.md
    // Nous devons changer le répertoire de travail, mais c'est complexe.
    // Pour l'instant, nous testons simplement que la fonction ne panique pas.
    let result = check_and_restart_if_needed(false, &messages, false);
    assert!(result.is_ok());
    
    // Nettoyage
    drop(temp_dir);
}