use crate::compaction::{CompactionConfig, compact_session};
use crate::context_pruning::{PruningConfig, prune_context};
use crate::failover::FailoverChain;
use crate::loop_detection::{LoopDetector, LoopStatus};
use crate::provider::{ChatRequest, LlmProvider};
use crate::thinking::ThinkingLevel;
use futures::StreamExt;
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
    pub thinking: ThinkingLevel,
    pub compaction: CompactionConfig,
    pub pruning: PruningConfig,
    pub max_context_tokens: u64,
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
            thinking: ThinkingLevel::Off,
            compaction: CompactionConfig::default(),
            pruning: PruningConfig::default(),
            max_context_tokens: 128000,
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
    // Compaction check
    let _compaction_result = compact_session(
        session, provider, &config.model, &config.compaction, config.max_context_tokens,
    ).await?;

    // Add user message to session
    session.add_message(Message::user(user_message));

    // Context pruning
    prune_context(&mut session.messages, &config.pruning);

    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let mut total_tool_calls = 0usize;
    let mut model_used = config.model.clone();
    let mut loop_detector = LoopDetector::new(20);

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
        let thinking_level = config.thinking.clone();

        info!("Agent round {} — calling LLM (model: {})", round + 1, config.model);

        let (response, used_model) = failover_chain
            .execute(|model, _key| {
                let model = model.to_string();
                let msgs = messages_clone.clone();
                let ts = tools_clone.clone();
                let tl = thinking_level.clone();
                async move {
                    let mut request = ChatRequest {
                        model,
                        messages: msgs,
                        tools: ts,
                        temperature,
                        max_tokens,
                        stream: false,
                        thinking: None,
                    };
                    tl.apply_to_request(&mut request);
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

            // Loop detection
            loop_detector.record(&tc.name, &tc.arguments);
        }

        // Check for loops after tool round
        match loop_detector.check() {
            LoopStatus::CircuitBreaker(msg) => {
                error!("Loop circuit breaker: {}", msg);
                let response = format!("I detected a loop in my tool usage and stopped. {}", msg);
                session.add_message(Message::assistant(&response));
                session.add_usage(total_input, total_output);
                return Ok(AgentResult {
                    response,
                    tool_calls_made: total_tool_calls,
                    input_tokens: total_input,
                    output_tokens: total_output,
                    model_used,
                });
            }
            LoopStatus::Critical(msg) | LoopStatus::Warning(msg) => {
                warn!("Loop detection: {}", msg);
                // Inject a hint to the LLM
                session.add_message(Message::system(&format!(
                    "[System warning: {}. Try a different approach.]", msg
                )));
            }
            LoopStatus::Ok => {}
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

/// Run agent with streaming — sends partial text chunks via `tx` channel
pub async fn run_agent_streaming(
    provider: &dyn LlmProvider,
    tools: &ToolRegistry,
    session: &mut Session,
    user_message: &str,
    config: &AgentConfig,
    tool_ctx: &ToolContext,
    tx: tokio::sync::mpsc::Sender<StreamChunk>,
) -> anyhow::Result<AgentResult> {
    // Compaction check
    let _compaction_result = compact_session(
        session, provider, &config.model, &config.compaction, config.max_context_tokens,
    ).await?;

    session.add_message(Message::user(user_message));
    prune_context(&mut session.messages, &config.pruning);

    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let mut total_tool_calls = 0usize;
    let mut model_used = config.model.clone();
    let mut loop_detector = LoopDetector::new(20);

    let tool_schemas = if !tools.is_empty() {
        Some(tools.tool_schemas())
    } else {
        None
    };

    for round in 0..config.max_tool_rounds {
        let mut messages = vec![Message::system(&config.system_prompt)];
        messages.extend(session.messages.clone());

        info!("Streaming agent round {} — calling LLM (model: {})", round + 1, config.model);

        let mut request = ChatRequest {
            model: config.model.clone(),
            messages,
            tools: tool_schemas.clone(),
            temperature: config.temperature,
            max_tokens: config.max_tokens,
            stream: true,
            thinking: None,
        };
        config.thinking.apply_to_request(&mut request);

        let mut stream = provider.chat_stream(request).await?;

        // Collect full response from stream
        let mut full_text = String::new();
        let mut collected_tool_calls: Vec<ToolCall> = Vec::new();
        let mut tool_call_args: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut usage = klaw_core::types::Usage::default();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    match &chunk {
                        StreamChunk::Text(text) => {
                            full_text.push_str(text);
                            let _ = tx.send(chunk.clone()).await;
                        }
                        StreamChunk::ToolCallStart { id, name } => {
                            collected_tool_calls.push(ToolCall {
                                id: id.clone(),
                                name: name.clone(),
                                arguments: serde_json::Value::Null,
                            });
                            tool_call_args.insert(id.clone(), String::new());
                        }
                        StreamChunk::ToolCallDelta { id, arguments } => {
                            if let Some(args) = tool_call_args.get_mut(id) {
                                args.push_str(arguments);
                            }
                        }
                        StreamChunk::ToolCallEnd { id } => {
                            if let Some(args_str) = tool_call_args.remove(id) {
                                if let Some(tc) = collected_tool_calls.iter_mut().find(|t| t.id == *id) {
                                    tc.arguments = serde_json::from_str(&args_str)
                                        .unwrap_or(serde_json::Value::String(args_str));
                                }
                            }
                        }
                        StreamChunk::Done { usage: u } => {
                            if let Some(u) = u {
                                usage = u.clone();
                            }
                        }
                        StreamChunk::Error(e) => {
                            warn!("Stream error: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Stream chunk error: {}", e);
                    break;
                }
            }
        }

        model_used = config.model.clone();
        total_input += usage.input_tokens;
        total_output += usage.output_tokens;

        // No tool calls — final response
        if collected_tool_calls.is_empty() {
            let content = if full_text.is_empty() {
                "I couldn't generate a response.".to_string()
            } else {
                full_text
            };
            session.add_message(Message::assistant(&content));
            session.add_usage(total_input, total_output);
            let _ = tx.send(StreamChunk::Done { usage: Some(usage) }).await;

            return Ok(AgentResult {
                response: content,
                tool_calls_made: total_tool_calls,
                input_tokens: total_input,
                output_tokens: total_output,
                model_used,
            });
        }

        // Process tool calls
        info!("Streaming round {} — {} tool call(s)", round + 1, collected_tool_calls.len());

        let mut assistant_msg = Message::assistant(&full_text);
        assistant_msg.tool_calls = Some(collected_tool_calls.clone());
        session.add_message(assistant_msg);

        for tc in &collected_tool_calls {
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

            let tool_msg = Message {
                id: uuid::Uuid::new_v4().to_string(),
                role: Role::Tool,
                content: result.content,
                tool_calls: None,
                tool_call_id: Some(result.tool_call_id),
                timestamp: chrono::Utc::now(),
            };
            session.add_message(tool_msg);

            loop_detector.record(&tc.name, &tc.arguments);
        }

        // Loop detection
        match loop_detector.check() {
            LoopStatus::CircuitBreaker(msg) => {
                error!("Loop circuit breaker: {}", msg);
                let response = format!("I detected a loop in my tool usage and stopped. {}", msg);
                session.add_message(Message::assistant(&response));
                session.add_usage(total_input, total_output);
                return Ok(AgentResult {
                    response,
                    tool_calls_made: total_tool_calls,
                    input_tokens: total_input,
                    output_tokens: total_output,
                    model_used,
                });
            }
            LoopStatus::Critical(msg) | LoopStatus::Warning(msg) => {
                warn!("Loop detection: {}", msg);
                session.add_message(Message::system(&format!(
                    "[System warning: {}. Try a different approach.]", msg
                )));
            }
            LoopStatus::Ok => {}
        }
    }

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
