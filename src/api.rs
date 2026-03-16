use serde::{Deserialize, Serialize};

// ------------------ Structures pour l'API DeepSeek ------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String, // "system", "user", "assistant", "tool"
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip)]
    pub token_count: Option<u32>, // Estimation ou comptage réel
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String, // "function"
    pub function: FunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // string JSON
}

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<Tool>,
    pub tool_choice: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stream: bool,
}

#[derive(Debug, Serialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Serialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    #[allow(dead_code)]
    pub completion_tokens: u32,
    #[allow(dead_code)]
    pub total_tokens: u32,
    pub prompt_cache_hit_tokens: Option<u32>,
    pub prompt_cache_miss_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: Message,
}

// Structures pour le streaming
#[derive(Debug, Deserialize)]
pub struct ChatChunk {
    pub choices: Vec<ChunkChoice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct ChunkChoice {
    pub delta: ChunkDelta,
}

#[derive(Debug, Deserialize)]
pub struct ChunkDelta {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

// Structures pour les tool_calls streaming (champs optionnels)
#[derive(Debug, Deserialize, Clone)]
pub struct ToolCallDelta {
    #[serde(default)]
    pub index: Option<u32>, // index dans le tableau tool_calls
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type", default)]
    pub call_type: Option<String>,
    #[serde(default)]
    pub function: Option<FunctionCallDelta>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FunctionCallDelta {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_message_serialization() {
        let msg = Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
            tool_calls: None,
            tool_call_id: None,
            token_count: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        // Vérifier que tool_calls et tool_call_id ne sont pas présents
        assert!(!json.contains("tool_calls"));
        assert!(!json.contains("tool_call_id"));
        // Vérifier les champs présents
        assert!(json.contains("role"));
        assert!(json.contains("user"));
        assert!(json.contains("content"));
        assert!(json.contains("Hello"));
        // Désérialiser pour vérifier
        let decoded: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.role, "user");
        assert_eq!(decoded.content, "Hello");
    }

    #[test]
    fn test_message_with_tool_calls() {
        let msg = Message {
            role: "assistant".to_string(),
            content: String::new(),
            tool_calls: Some(vec![ToolCall {
                id: "call_123".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "sh".to_string(),
                    arguments: "{\"command\": \"ls\"}".to_string(),
                },
            }]),
            tool_call_id: None,
            token_count: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("tool_calls"));
        assert!(json.contains("call_123"));
        assert!(json.contains("sh"));
        // tool_call_id absent
        assert!(!json.contains("tool_call_id"));
    }

    #[test]
    fn test_message_with_tool_call_id() {
        let msg = Message {
            role: "tool".to_string(),
            content: "output".to_string(),
            tool_calls: None,
            tool_call_id: Some("call_123".to_string()),
            token_count: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        // tool_call_id doit être présent pour les rôles "tool"? 
        // Selon la documentation de l'API, tool_call_id est requis pour les messages de rôle "tool".
        // Notre sérialiseur inclut tool_call_id s'il est Some (skip_serializing_if = "Option::is_none").
        // Donc il doit être présent.
        assert!(json.contains("tool_call_id"));
        assert!(json.contains("call_123"));
    }

    #[test]
    fn test_chat_request_serialization() {
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
                    parameters: serde_json::json!({}),
                },
            }],
            tool_choice: "auto".to_string(),
            stream: false,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("deepseek-chat"));
        assert!(json.contains("tools"));
        assert!(json.contains("sh"));
        // stream: false ne doit pas être présent (skip_serializing_if)
        assert!(!json.contains("stream"));
    }

    #[test]
    fn test_chat_request_stream_true() {
        let request = ChatRequest {
            model: "deepseek-chat".to_string(),
            messages: vec![],
            tools: vec![],
            tool_choice: "auto".to_string(),
            stream: true,
        };
        let json = serde_json::to_string(&request).unwrap();
        // stream: true doit être présent
        assert!(json.contains("stream"));
        assert!(json.contains("true"));
    }

    #[test]
    fn test_chat_response_deserialization() {
        let json = r#"{
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello there"
                }
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15,
                "prompt_cache_hit_tokens": 2,
                "prompt_cache_miss_tokens": 8
            }
        }"#;
        let response: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.choices[0].message.role, "assistant");
        assert_eq!(response.choices[0].message.content, "Hello there");
        assert_eq!(response.usage.prompt_tokens, 10);
        assert_eq!(response.usage.completion_tokens, 5);
        assert_eq!(response.usage.total_tokens, 15);
        assert_eq!(response.usage.prompt_cache_hit_tokens, Some(2));
        assert_eq!(response.usage.prompt_cache_miss_tokens, Some(8));
    }

    #[test]
    fn test_chat_chunk_deserialization() {
        let json = r#"{
            "choices": [{
                "delta": {
                    "content": "Hello"
                }
            }]
        }"#;
        let chunk: ChatChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.choices.len(), 1);
        assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
        assert!(chunk.choices[0].delta.role.is_none());
        assert!(chunk.choices[0].delta.tool_calls.is_none());
    }

    #[test]
    fn test_chat_chunk_with_tool_calls() {
        let json = r#"{
            "choices": [{
                "delta": {
                    "tool_calls": [
                        {
                            "index": 0,
                            "id": "call_123",
                            "type": "function",
                            "function": {
                                "name": "sh",
                                "arguments": "{\"command\": \"ls\"}"
                            }
                        }
                    ]
                }
            }]
        }"#;
        let chunk: ChatChunk = serde_json::from_str(json).unwrap();
        let tool_calls = chunk.choices[0].delta.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].index, Some(0));
        assert_eq!(tool_calls[0].id, Some("call_123".to_string()));
        assert_eq!(tool_calls[0].call_type, Some("function".to_string()));
        let function = tool_calls[0].function.as_ref().unwrap();
        assert_eq!(function.name, Some("sh".to_string()));
        assert_eq!(function.arguments, Some("{\"command\": \"ls\"}".to_string()));
    }
}
