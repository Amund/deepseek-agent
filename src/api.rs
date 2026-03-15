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
