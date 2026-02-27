use crate::provider::{ChatRequest, LlmProvider};
use klaw_core::session::Session;
use klaw_core::types::{Message, Role, StreamChunk, ToolCall, ToolResult};
use klaw_tools::{Tool, ToolContext, registry::ToolRegistry};
use std::sync::Arc;
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
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "gpt-4".to_string(),
            system_prompt: String::new(),
            max_tool_rounds: MAX_TOOL_ROUNDS,
            temperature: None,
            max_tokens: None,
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

        // Call LLM
        let request = ChatRequest {
            model: config.model.clone(),
            messages,
            tools: tool_schemas.clone(),
            temperature: config.temperature,
            max_tokens: config.max_tokens,
            stream: false,
        };

        info!("Agent round {} — calling LLM (model: {})", round + 1, config.model);
        let response = provider.chat(request).await?;

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
    })
}
