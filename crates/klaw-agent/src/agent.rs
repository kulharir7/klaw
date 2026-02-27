use crate::failover::FailoverChain;
use crate::provider::{ChatRequest, LlmProvider};
use klaw_core::session::Session;
use klaw_core::types::{Message, Role, StreamChunk, ToolCall, ToolResult};
use klaw_tools::{Tool, ToolContext, registry::ToolRegistry};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn, error};

/// Maximum tool call rounds to prevent infinite loops
const MAX_TOOL_ROUNDS: usize = 10;

/// Agent configuration
pub struct AgentConfig {
    pub model: String,
    pub system_prompt: String,
    pub max_tool_rounds: usize,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub failover_models: Option<Vec<String>>,
    pub api_keys: Option<Vec<String>>,
    pub retry_count: u32,
    pub retry_delay: Duration,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "gpt-4".to_string(),
            system_prompt: String::new(),
            max_tool_rounds: MAX_TOOL_ROUNDS,
            temperature: None,
            max_tokens: None,
            failover_models: None,
            api_keys: None,
            retry_count: 2,
            retry_delay: Duration::from_millis(1000),
        }
    }
}

/// Run result from agent
#[derive(Debug)]
pub struct AgentResult {
    pub response: String,
    pub tool_calls_made: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub model_used: String,
}

/// Run the agent loop: message → LLM → tool calls → response
pub async fn run_agent(
    provider: &dyn LlmProvider,
    tools: &ToolRegistry,
    session: &mut Session,
    user_message: &str,
    config: &AgentConfig,
    tool_ctx: &ToolContext,
) -> anyhow::Result<AgentResult> {
    // Add user message to session
    session.add_message(Message::user(user_message));

    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let mut total_tool_calls = 0usize;
    let mut model_used = config.model.clone();

    // Build failover chain
    let failover_chain = FailoverChain::new(
        &config.model,
        config.failover_models.as_deref(),
        config.api_keys.as_deref(),
        config.retry_count,
        config.retry_delay,
    );

    // Build tool schemas
    let tool_schemas = if !tools.is_empty() {
        Some(tools.tool_schemas())
    } else {
        None
    };

    for round in 0..config.max_tool_rounds {
        // Build messages array
        let mut messages = vec![Message::system(&config.system_prompt)];
        messages.extend(session.messages.clone());

        // Call LLM with failover
        let messages_clone = messages.clone();
        let tools_clone = tool_schemas.clone();
        let temperature = config.temperature;
        let max_tokens = config.max_tokens;

        info!("Agent round {} — calling LLM (model: {})", round + 1, config.model);

        let (response, used_model) = failover_chain
            .execute(|model, _key| {
                let model = model.to_string();
                let msgs = messages_clone.clone();
                let ts = tools_clone.clone();
                async move {
                    let request = ChatRequest {
                        model,
                        messages: msgs,
                        tools: ts,
                        temperature,
                        max_tokens,
                        stream: false,
                    };
                    provider.chat(request).await
                }
            })
            .await?;
        model_used = used_model;

        total_input += response.usage.input_tokens;
        total_output += response.usage.output_tokens;

        // No tool calls — we have the final response
        if response.tool_calls.is_empty() {
            let content = response.content.unwrap_or_else(|| "I couldn't generate a response.".to_string());
            session.add_message(Message::assistant(&content));
            session.add_usage(total_input, total_output);

            return Ok(AgentResult {
                response: content,
                tool_calls_made: total_tool_calls,
                input_tokens: total_input,
                output_tokens: total_output,
                model_used: model_used.clone(),
            });
        }

        // Process tool calls
        info!("Agent round {} — {} tool call(s)", round + 1, response.tool_calls.len());

        // Add assistant message with tool calls
        let mut assistant_msg = Message::assistant(response.content.as_deref().unwrap_or(""));
        assistant_msg.tool_calls = Some(response.tool_calls.clone());
        session.add_message(assistant_msg);

        // Execute each tool
        for tc in &response.tool_calls {
            total_tool_calls += 1;
            info!("Executing tool: {} (id: {})", tc.name, tc.id);

            let result = match tools.get(&tc.name) {
                Some(tool) => {
                    match tool.execute(tc.arguments.clone(), tool_ctx).await {
                        Ok(r) => r,
                        Err(e) => {
                            warn!("Tool {} failed: {}", tc.name, e);
                            ToolResult {
                                tool_call_id: tc.id.clone(),
                                content: format!("Error: {}", e),
                                is_error: true,
                            }
                        }
                    }
                }
                None => {
                    warn!("Unknown tool: {}", tc.name);
                    ToolResult {
                        tool_call_id: tc.id.clone(),
                        content: format!("Error: Unknown tool '{}'", tc.name),
                        is_error: true,
                    }
                }
            };

            // Add tool result to session
            let mut tool_msg = Message {
                id: uuid::Uuid::new_v4().to_string(),
                role: Role::Tool,
                content: result.content,
                tool_calls: None,
                tool_call_id: Some(result.tool_call_id),
                timestamp: chrono::Utc::now(),
            };
            session.add_message(tool_msg);
        }
    }

    // Hit max rounds
    let fallback = "I've reached the maximum number of tool call rounds. Here's what I have so far.".to_string();
    session.add_message(Message::assistant(&fallback));
    session.add_usage(total_input, total_output);

    Ok(AgentResult {
        response: fallback,
        tool_calls_made: total_tool_calls,
        input_tokens: total_input,
        output_tokens: total_output,
        model_used,
    })
}
