//! Message types for Claude Agent SDK

use serde::{Deserialize, Serialize};

/// Supported image MIME types for Claude API
const SUPPORTED_IMAGE_MIME_TYPES: &[&str] = &["image/jpeg", "image/png", "image/gif", "image/webp"];

/// Maximum base64 data size (15MB results in ~20MB decoded, within Claude's limits)
const MAX_BASE64_SIZE: usize = 15_728_640;

/// Error types for assistant messages
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssistantMessageError {
    /// Authentication failed
    AuthenticationFailed,
    /// Billing error
    BillingError,
    /// Rate limit exceeded
    RateLimit,
    /// Invalid request
    InvalidRequest,
    /// Server error
    ServerError,
    /// Unknown error
    Unknown,
}

/// Main message enum containing all message types from CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Message {
    /// Assistant message
    #[serde(rename = "assistant")]
    Assistant(AssistantMessage),
    /// System message
    #[serde(rename = "system")]
    System(SystemMessage),
    /// Result message
    #[serde(rename = "result")]
    Result(ResultMessage),
    /// Stream event
    #[serde(rename = "stream_event")]
    StreamEvent(StreamEvent),
    /// User message (rarely used in stream output)
    #[serde(rename = "user")]
    User(UserMessage),
    /// Control cancel request (ignore this - it's internal control protocol)
    #[serde(rename = "control_cancel_request")]
    ControlCancelRequest(serde_json::Value),
    /// Rate limit status event emitted by newer CLI versions
    #[serde(rename = "rate_limit_event")]
    RateLimitEvent(RateLimitEvent),
}

/// User message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    /// Message text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Message content blocks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<ContentBlock>>,
    /// UUID for file checkpointing (used with enable_file_checkpointing and rewind_files)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// Parent tool use ID (if this is a tool result)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<String>,
    /// Additional fields
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Message content can be text or blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Simple text content
    Text { text: String },
    /// Structured content blocks
    Blocks { content: Vec<ContentBlock> },
}

impl From<String> for MessageContent {
    fn from(text: String) -> Self {
        MessageContent::Text { text }
    }
}

impl From<&str> for MessageContent {
    fn from(text: &str) -> Self {
        MessageContent::Text {
            text: text.to_string(),
        }
    }
}

impl From<Vec<ContentBlock>> for MessageContent {
    fn from(blocks: Vec<ContentBlock>) -> Self {
        MessageContent::Blocks { content: blocks }
    }
}

/// Assistant message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    /// The actual message content (wrapped)
    pub message: AssistantMessageInner,
    /// Parent tool use ID (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<String>,
    /// Session ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// UUID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
}

/// Typed token usage as reported by the CLI (`usage` on assistant and
/// result messages). Every field defaults to 0 so partial or future payloads
/// still parse; the raw JSON stays available in the `usage` field.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Uncached input tokens
    #[serde(default, deserialize_with = "null_as_zero")]
    pub input_tokens: u64,
    /// Output tokens
    #[serde(default, deserialize_with = "null_as_zero")]
    pub output_tokens: u64,
    /// Tokens written to the prompt cache
    #[serde(default, deserialize_with = "null_as_zero")]
    pub cache_creation_input_tokens: u64,
    /// Tokens read from the prompt cache
    #[serde(default, deserialize_with = "null_as_zero")]
    pub cache_read_input_tokens: u64,
}

/// The CLI reports unset cache counters as `null`; treat that as 0.
fn null_as_zero<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    Ok(Option::<u64>::deserialize(d)?.unwrap_or(0))
}

impl TokenUsage {
    /// Parse from the raw `usage` JSON; `None` if it is not an object of the expected shape.
    pub fn from_value(v: &serde_json::Value) -> Option<Self> {
        serde_json::from_value(v.clone()).ok()
    }

    /// Sum of all input-side tokens (uncached + cache writes + cache reads).
    pub fn total_input_tokens(&self) -> u64 {
        self.input_tokens + self.cache_creation_input_tokens + self.cache_read_input_tokens
    }

    /// Input plus output tokens.
    pub fn total_tokens(&self) -> u64 {
        self.total_input_tokens() + self.output_tokens
    }
}

impl std::ops::AddAssign for TokenUsage {
    fn add_assign(&mut self, o: Self) {
        self.input_tokens += o.input_tokens;
        self.output_tokens += o.output_tokens;
        self.cache_creation_input_tokens += o.cache_creation_input_tokens;
        self.cache_read_input_tokens += o.cache_read_input_tokens;
    }
}

impl std::ops::Add for TokenUsage {
    type Output = Self;
    fn add(mut self, o: Self) -> Self {
        self += o;
        self
    }
}

impl AssistantMessageInner {
    /// Typed view of `usage`, if present.
    pub fn token_usage(&self) -> Option<TokenUsage> {
        self.usage.as_ref().and_then(TokenUsage::from_value)
    }
}

impl ResultMessage {
    /// Typed view of `usage`, if present.
    pub fn token_usage(&self) -> Option<TokenUsage> {
        self.usage.as_ref().and_then(TokenUsage::from_value)
    }
}

/// Per-model usage statistics from a result message (`modelUsage`).
/// The CLI uses camelCase keys here. `output_tokens` already includes
/// `thinking_tokens`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    /// Uncached input tokens
    #[serde(default)]
    pub input_tokens: u64,
    /// Output tokens (including thinking tokens)
    #[serde(default)]
    pub output_tokens: u64,
    /// Thinking tokens generated by this model, when reported
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_tokens: Option<u64>,
    /// Tokens read from the prompt cache
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    /// Tokens written to the prompt cache
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    /// Number of web search requests
    #[serde(default)]
    pub web_search_requests: u64,
    /// Client-side cost estimate in USD
    #[serde(default, rename = "costUSD")]
    pub cost_usd: f64,
    /// Model context window size
    #[serde(default)]
    pub context_window: u64,
    /// Model max output tokens
    #[serde(default)]
    pub max_output_tokens: u64,
    /// Canonical model ID used for pricing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_model: Option<String>,
    /// API backend that served the model (`firstParty`, `bedrock`, ...)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Price table used (`list`, `managed`, `unknown`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_basis: Option<String>,
}

/// Rate limit status reported by a `rate_limit_event`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitStatus {
    /// Within limits
    Allowed,
    /// Approaching a limit
    AllowedWarning,
    /// Request rejected by a limit
    Rejected,
    /// Status added by a newer CLI
    #[serde(other)]
    Unknown,
}

/// Details of a rate limit event (`rate_limit_info`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitInfo {
    /// Current status
    pub status: RateLimitStatus,
    /// Unix timestamp when the limit resets
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<f64>,
    /// Utilization of the limit (0.0 - 1.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utilization: Option<f64>,
    /// Set to `credits_required` when included usage is exhausted
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// Whether the user can buy usage credits
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_user_purchase_credits: Option<bool>,
    /// Whether a chargeable saved payment method is on file
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_chargeable_saved_payment_method: Option<bool>,
}

/// `rate_limit_event` message (the `type` tag lives on [`Message`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitEvent {
    /// Rate limit details
    pub rate_limit_info: RateLimitInfo,
    /// Event UUID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// Session ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Inner assistant message content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessageInner {
    /// Message content blocks
    #[serde(default)]
    pub content: Vec<ContentBlock>,
    /// Model used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Message ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Stop reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// Usage statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<serde_json::Value>,
    /// Error type (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AssistantMessageError>,
}

/// System message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMessage {
    /// Message subtype
    pub subtype: String,
    /// Current working directory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Session ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Available tools
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    /// MCP servers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<serde_json::Value>>,
    /// Model being used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Permission mode
    #[serde(skip_serializing_if = "Option::is_none", rename = "permissionMode")]
    pub permission_mode: Option<String>,
    /// UUID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// Additional data
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// Result message indicating query completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultMessage {
    /// Result subtype
    pub subtype: String,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// API duration in milliseconds
    pub duration_api_ms: u64,
    /// Whether this is an error result
    pub is_error: bool,
    /// Number of turns in conversation
    pub num_turns: u32,
    /// Session ID
    pub session_id: String,
    /// Total cost in USD
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost_usd: Option<f64>,
    /// Usage statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<serde_json::Value>,
    /// Per-model usage and cost, keyed by model name
    #[serde(default, rename = "modelUsage", skip_serializing_if = "Option::is_none")]
    pub model_usage: Option<std::collections::HashMap<String, ModelUsage>>,
    /// Result text (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// Structured output (when output_format is specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<serde_json::Value>,
}

/// Stream event message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    /// Event UUID
    pub uuid: String,
    /// Session ID
    pub session_id: String,
    /// Event data
    pub event: serde_json::Value,
    /// Parent tool use ID (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<String>,
}

/// Content block types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Text block
    Text(TextBlock),
    /// Thinking block (extended thinking)
    Thinking(ThinkingBlock),
    /// Tool use block
    ToolUse(ToolUseBlock),
    /// Tool result block
    ToolResult(ToolResultBlock),
    /// Image block
    Image(ImageBlock),
}

/// Text content block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    /// Text content
    pub text: String,
}

/// Thinking block (extended thinking)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingBlock {
    /// Thinking content
    pub thinking: String,
    /// Signature
    pub signature: String,
}

/// Tool use block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseBlock {
    /// Tool use ID
    pub id: String,
    /// Tool name
    pub name: String,
    /// Tool input parameters
    pub input: serde_json::Value,
}

/// Tool result block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultBlock {
    /// Tool use ID this result corresponds to
    pub tool_use_id: String,
    /// Result content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<ToolResultContent>,
    /// Whether this is an error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Tool result content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    /// Text result
    Text(String),
    /// Structured blocks
    Blocks(Vec<serde_json::Value>),
}

/// Image source for user prompts
///
/// Represents the source of image data that can be included in user messages.
/// Claude supports both base64-encoded images and URL references.
///
/// # Supported Formats
///
/// - JPEG (`image/jpeg`)
/// - PNG (`image/png`)
/// - GIF (`image/gif`)
/// - WebP (`image/webp`)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    /// Base64-encoded image data
    Base64 {
        /// MIME type (e.g., "image/png", "image/jpeg", "image/gif", "image/webp")
        media_type: String,
        /// Base64-encoded image data (without data URI prefix)
        data: String,
    },
    /// URL reference to an image
    Url {
        /// Publicly accessible image URL
        url: String,
    },
}

/// Image block for user prompts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageBlock {
    /// Image source (base64 or URL)
    pub source: ImageSource,
}

/// Content block for user prompts (input)
///
/// Represents content that can be included in user messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserContentBlock {
    /// Text content
    Text {
        /// Text content string
        text: String,
    },
    /// Image content
    Image {
        /// Image source (base64 or URL)
        source: ImageSource,
    },
}

impl UserContentBlock {
    /// Create a text content block
    pub fn text(text: impl Into<String>) -> Self {
        UserContentBlock::Text { text: text.into() }
    }

    /// Create an image content block from base64 data
    ///
    /// # Arguments
    ///
    /// * `media_type` - MIME type of the image (e.g., "image/png", "image/jpeg")
    /// * `data` - Base64-encoded image data (without data URI prefix)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The MIME type is not supported (valid types: image/jpeg, image/png, image/gif, image/webp)
    /// - The base64 data exceeds the maximum size limit (15MB)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use claude_agent_sdk_rs::UserContentBlock;
    /// let block = UserContentBlock::image_base64("image/png", "iVBORw0KGgo=")?;
    /// # Ok::<(), claude_agent_sdk_rs::ClaudeError>(())
    /// ```
    pub fn image_base64(
        media_type: impl Into<String>,
        data: impl Into<String>,
    ) -> crate::errors::Result<Self> {
        let media_type_str = media_type.into();
        let data_str = data.into();

        // Validate MIME type
        if !SUPPORTED_IMAGE_MIME_TYPES.contains(&media_type_str.as_str()) {
            return Err(crate::errors::ImageValidationError::new(format!(
                "Unsupported media type '{}'. Supported types: {:?}",
                media_type_str, SUPPORTED_IMAGE_MIME_TYPES
            ))
            .into());
        }

        // Validate base64 size
        if data_str.len() > MAX_BASE64_SIZE {
            return Err(crate::errors::ImageValidationError::new(format!(
                "Base64 data exceeds maximum size of {} bytes (got {} bytes)",
                MAX_BASE64_SIZE,
                data_str.len()
            ))
            .into());
        }

        Ok(UserContentBlock::Image {
            source: ImageSource::Base64 {
                media_type: media_type_str,
                data: data_str,
            },
        })
    }

    /// Create an image content block from URL
    pub fn image_url(url: impl Into<String>) -> Self {
        UserContentBlock::Image {
            source: ImageSource::Url { url: url.into() },
        }
    }

    /// Validate a collection of content blocks
    ///
    /// Ensures the content is non-empty. This is used internally by query functions
    /// to provide consistent validation.
    ///
    /// # Errors
    ///
    /// Returns an error if the content blocks slice is empty.
    pub fn validate_content(blocks: &[UserContentBlock]) -> crate::Result<()> {
        if blocks.is_empty() {
            return Err(crate::errors::ClaudeError::InvalidConfig(
                "Content must include at least one block (text or image)".to_string(),
            ));
        }
        Ok(())
    }
}

impl From<String> for UserContentBlock {
    fn from(text: String) -> Self {
        UserContentBlock::Text { text }
    }
}

impl From<&str> for UserContentBlock {
    fn from(text: &str) -> Self {
        UserContentBlock::Text {
            text: text.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_token_usage() {
        let u = TokenUsage::from_value(&json!({
            "input_tokens": 3, "output_tokens": 5,
            "cache_creation_input_tokens": 7, "cache_read_input_tokens": 11,
            "cache_creation": {"ephemeral_5m_input_tokens": 7, "ephemeral_1h_input_tokens": 0},
            "service_tier": "standard"
        }))
        .unwrap();
        assert_eq!(u.total_input_tokens(), 21);
        assert_eq!(u.total_tokens(), 26);
        assert_eq!((u + u).output_tokens, 10);
        assert_eq!(TokenUsage::from_value(&json!({})).unwrap(), TokenUsage::default());
        let n = TokenUsage::from_value(&json!({"input_tokens": 1, "cache_read_input_tokens": null})).unwrap();
        assert_eq!(n.total_tokens(), 1);
    }

    #[test]
    fn test_parse_rate_limit_event() {
        let msg: Message = serde_json::from_value(json!({
            "type": "rate_limit_event",
            "rate_limit_info": {"status": "allowed_warning", "resetsAt": 1760000000, "utilization": 0.9},
            "uuid": "u", "session_id": "s"
        }))
        .unwrap();
        let Message::RateLimitEvent(e) = msg else { panic!("wrong variant") };
        assert!(matches!(e.rate_limit_info.status, RateLimitStatus::AllowedWarning));
        assert_eq!(e.rate_limit_info.utilization, Some(0.9));
    }

    #[test]
    fn test_parse_result_model_usage() {
        let msg: Message = serde_json::from_value(json!({
            "type": "result", "subtype": "success", "duration_ms": 1, "duration_api_ms": 1,
            "is_error": false, "num_turns": 1, "session_id": "s",
            "usage": {"input_tokens": 2, "output_tokens": 3},
            "modelUsage": {"claude-x": {
                "inputTokens": 2, "outputTokens": 3, "cacheReadInputTokens": 0,
                "cacheCreationInputTokens": 0, "webSearchRequests": 0, "costUSD": 0.01,
                "contextWindow": 200000, "maxOutputTokens": 64000
            }}
        }))
        .unwrap();
        let Message::Result(r) = msg else { panic!("wrong variant") };
        let m = &r.model_usage.unwrap()["claude-x"];
        assert_eq!(m.context_window, 200000);
        assert!((m.cost_usd - 0.01).abs() < 1e-9);
    }

    #[test]
    fn test_content_block_text_serialization() {
        let block = ContentBlock::Text(TextBlock {
            text: "Hello".to_string(),
        });

        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello");
    }

    #[test]
    fn test_content_block_tool_use_serialization() {
        let block = ContentBlock::ToolUse(ToolUseBlock {
            id: "tool_123".to_string(),
            name: "Bash".to_string(),
            input: json!({"command": "echo hello"}),
        });

        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "tool_use");
        assert_eq!(json["id"], "tool_123");
        assert_eq!(json["name"], "Bash");
        assert_eq!(json["input"]["command"], "echo hello");
    }

    #[test]
    fn test_message_assistant_deserialization() {
        let json_str = r#"{
            "type": "assistant",
            "message": {
                "content": [{"type": "text", "text": "Hello"}],
                "model": "claude-sonnet-4"
            },
            "session_id": "test-session"
        }"#;

        let msg: Message = serde_json::from_str(json_str).unwrap();
        match msg {
            Message::Assistant(assistant) => {
                assert_eq!(assistant.session_id, Some("test-session".to_string()));
                assert_eq!(assistant.message.model, Some("claude-sonnet-4".to_string()));
            }
            _ => panic!("Expected Assistant variant"),
        }
    }

    #[test]
    fn test_message_result_deserialization() {
        let json_str = r#"{
            "type": "result",
            "subtype": "query_complete",
            "duration_ms": 1500,
            "duration_api_ms": 1200,
            "is_error": false,
            "num_turns": 3,
            "session_id": "test-session",
            "total_cost_usd": 0.0042
        }"#;

        let msg: Message = serde_json::from_str(json_str).unwrap();
        match msg {
            Message::Result(result) => {
                assert_eq!(result.subtype, "query_complete");
                assert_eq!(result.duration_ms, 1500);
                assert_eq!(result.num_turns, 3);
                assert_eq!(result.total_cost_usd, Some(0.0042));
            }
            _ => panic!("Expected Result variant"),
        }
    }

    #[test]
    fn test_message_system_deserialization() {
        let json_str = r#"{
            "type": "system",
            "subtype": "session_start",
            "cwd": "/home/user",
            "session_id": "test-session",
            "tools": ["Bash", "Read", "Write"]
        }"#;

        let msg: Message = serde_json::from_str(json_str).unwrap();
        match msg {
            Message::System(system) => {
                assert_eq!(system.subtype, "session_start");
                assert_eq!(system.cwd, Some("/home/user".to_string()));
                assert_eq!(system.tools.as_ref().unwrap().len(), 3);
            }
            _ => panic!("Expected System variant"),
        }
    }

    #[test]
    fn test_tool_result_content_text() {
        let content = ToolResultContent::Text("Command output".to_string());
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json, "Command output");
    }

    #[test]
    fn test_tool_result_content_blocks() {
        let content = ToolResultContent::Blocks(vec![json!({"type": "text", "text": "Result"})]);
        let json = serde_json::to_value(&content).unwrap();
        assert!(json.is_array());
        assert_eq!(json[0]["type"], "text");
    }

    #[test]
    fn test_image_source_base64_serialization() {
        let source = ImageSource::Base64 {
            media_type: "image/png".to_string(),
            data: "iVBORw0KGgo=".to_string(),
        };

        let json = serde_json::to_value(&source).unwrap();
        assert_eq!(json["type"], "base64");
        assert_eq!(json["media_type"], "image/png");
        assert_eq!(json["data"], "iVBORw0KGgo=");
    }

    #[test]
    fn test_image_source_url_serialization() {
        let source = ImageSource::Url {
            url: "https://example.com/image.png".to_string(),
        };

        let json = serde_json::to_value(&source).unwrap();
        assert_eq!(json["type"], "url");
        assert_eq!(json["url"], "https://example.com/image.png");
    }

    #[test]
    fn test_image_source_base64_deserialization() {
        let json_str = r#"{
            "type": "base64",
            "media_type": "image/jpeg",
            "data": "base64data=="
        }"#;

        let source: ImageSource = serde_json::from_str(json_str).unwrap();
        match source {
            ImageSource::Base64 { media_type, data } => {
                assert_eq!(media_type, "image/jpeg");
                assert_eq!(data, "base64data==");
            }
            _ => panic!("Expected Base64 variant"),
        }
    }

    #[test]
    fn test_image_source_url_deserialization() {
        let json_str = r#"{
            "type": "url",
            "url": "https://example.com/test.gif"
        }"#;

        let source: ImageSource = serde_json::from_str(json_str).unwrap();
        match source {
            ImageSource::Url { url } => {
                assert_eq!(url, "https://example.com/test.gif");
            }
            _ => panic!("Expected Url variant"),
        }
    }

    #[test]
    fn test_user_content_block_text_serialization() {
        let block = UserContentBlock::text("Hello world");

        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello world");
    }

    #[test]
    fn test_user_content_block_image_base64_serialization() {
        let block = UserContentBlock::image_base64("image/png", "iVBORw0KGgo=").unwrap();

        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "image");
        assert_eq!(json["source"]["type"], "base64");
        assert_eq!(json["source"]["media_type"], "image/png");
        assert_eq!(json["source"]["data"], "iVBORw0KGgo=");
    }

    #[test]
    fn test_user_content_block_image_url_serialization() {
        let block = UserContentBlock::image_url("https://example.com/image.webp");

        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "image");
        assert_eq!(json["source"]["type"], "url");
        assert_eq!(json["source"]["url"], "https://example.com/image.webp");
    }

    #[test]
    fn test_user_content_block_from_string() {
        let block: UserContentBlock = "Test message".into();

        match block {
            UserContentBlock::Text { text } => {
                assert_eq!(text, "Test message");
            }
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_user_content_block_from_owned_string() {
        let block: UserContentBlock = String::from("Owned message").into();

        match block {
            UserContentBlock::Text { text } => {
                assert_eq!(text, "Owned message");
            }
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_image_block_serialization() {
        let block = ImageBlock {
            source: ImageSource::Base64 {
                media_type: "image/gif".to_string(),
                data: "R0lGODlh".to_string(),
            },
        };

        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["source"]["type"], "base64");
        assert_eq!(json["source"]["media_type"], "image/gif");
        assert_eq!(json["source"]["data"], "R0lGODlh");
    }

    #[test]
    fn test_content_block_image_serialization() {
        let block = ContentBlock::Image(ImageBlock {
            source: ImageSource::Url {
                url: "https://example.com/photo.jpg".to_string(),
            },
        });

        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "image");
        assert_eq!(json["source"]["type"], "url");
        assert_eq!(json["source"]["url"], "https://example.com/photo.jpg");
    }

    #[test]
    fn test_content_block_image_deserialization() {
        let json_str = r#"{
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": "image/webp",
                "data": "UklGR"
            }
        }"#;

        let block: ContentBlock = serde_json::from_str(json_str).unwrap();
        match block {
            ContentBlock::Image(image) => match image.source {
                ImageSource::Base64 { media_type, data } => {
                    assert_eq!(media_type, "image/webp");
                    assert_eq!(data, "UklGR");
                }
                _ => panic!("Expected Base64 source"),
            },
            _ => panic!("Expected Image variant"),
        }
    }

    #[test]
    fn test_user_content_block_deserialization() {
        let json_str = r#"{
            "type": "text",
            "text": "Describe this image"
        }"#;

        let block: UserContentBlock = serde_json::from_str(json_str).unwrap();
        match block {
            UserContentBlock::Text { text } => {
                assert_eq!(text, "Describe this image");
            }
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_user_content_block_image_deserialization() {
        let json_str = r#"{
            "type": "image",
            "source": {
                "type": "url",
                "url": "https://example.com/diagram.png"
            }
        }"#;

        let block: UserContentBlock = serde_json::from_str(json_str).unwrap();
        match block {
            UserContentBlock::Image { source } => match source {
                ImageSource::Url { url } => {
                    assert_eq!(url, "https://example.com/diagram.png");
                }
                _ => panic!("Expected Url source"),
            },
            _ => panic!("Expected Image variant"),
        }
    }

    #[test]
    fn test_image_base64_valid() {
        let block = UserContentBlock::image_base64("image/png", "iVBORw0KGgo=");
        assert!(block.is_ok());
    }

    #[test]
    fn test_image_base64_invalid_mime_type() {
        let block = UserContentBlock::image_base64("image/bmp", "data");
        assert!(block.is_err());
        let err = block.unwrap_err().to_string();
        assert!(err.contains("Unsupported media type"));
        assert!(err.contains("image/bmp"));
    }

    #[test]
    fn test_image_base64_exceeds_size_limit() {
        let large_data = "a".repeat(MAX_BASE64_SIZE + 1);
        let block = UserContentBlock::image_base64("image/png", large_data);
        assert!(block.is_err());
        let err = block.unwrap_err().to_string();
        assert!(err.contains("exceeds maximum size"));
    }
}
