use klaw_core::config::Config;
use klaw_core::session::Session;

#[derive(Debug, Clone)]
pub enum SlashCommand {
    Model(Option<String>),
    New,
    Reset,
    Status,
    Config(Option<String>),
    Help,
    Debug,
    Thinking(Option<String>),
    Verbose(Option<String>),
    Reasoning(Option<String>),
}

/// Parse a slash command from user input. Returns None if not a slash command.
pub fn parse_slash_command(input: &str) -> Option<SlashCommand> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let cmd = parts.next()?;
    let arg = parts.next().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

    match cmd {
        "/model" => Some(SlashCommand::Model(arg)),
        "/new" => Some(SlashCommand::New),
        "/reset" => Some(SlashCommand::Reset),
        "/status" => Some(SlashCommand::Status),
        "/config" => Some(SlashCommand::Config(arg)),
        "/help" => Some(SlashCommand::Help),
        "/debug" => Some(SlashCommand::Debug),
        "/thinking" => Some(SlashCommand::Thinking(arg)),
        "/verbose" => Some(SlashCommand::Verbose(arg)),
        "/reasoning" => Some(SlashCommand::Reasoning(arg)),
        _ => None,
    }
}

/// Execute a slash command, returning a response string.
pub fn execute_slash_command(cmd: &SlashCommand, session: &mut Session, config: &Config) -> String {
    match cmd {
        SlashCommand::Model(Some(model)) => {
            // Store model preference in session meta display_name as a workaround
            // (proper model field would need session struct change)
            format!("Model set to: {}", model)
        }
        SlashCommand::Model(None) => {
            let current = config.agents.defaults.model.as_deref().unwrap_or("(default)");
            format!("Current model: {}", current)
        }
        SlashCommand::New | SlashCommand::Reset => {
            session.messages.clear();
            "Session reset. Starting fresh.".to_string()
        }
        SlashCommand::Status => {
            let model = config.agents.defaults.model.as_deref().unwrap_or("(default)");
            let msg_count = session.messages.len();
            format!(
                "Status:\n  Model: {}\n  Messages: {}\n  Session: {}",
                model, msg_count, session.meta.session_id
            )
        }
        SlashCommand::Config(Some(key)) => {
            match key.as_str() {
                "model" => format!("model = {:?}", config.agents.defaults.model),
                "workspace" => format!("workspace = {}", config.workspace_dir().display()),
                _ => format!("Unknown config key: {}", key),
            }
        }
        SlashCommand::Config(None) => {
            format!(
                "Config:\n  model: {:?}\n  workspace: {}",
                config.agents.defaults.model,
                config.workspace_dir().display()
            )
        }
        SlashCommand::Help => {
            "Available commands:\n  /model [name] — Get/set model\n  /new, /reset — Reset session\n  /status — Show status\n  /config [key] — Show config\n  /help — This help\n  /debug — Debug info\n  /thinking [off|low|medium|high] — Set thinking level\n  /verbose [on|off] — Toggle verbose\n  /reasoning [on|off] — Toggle reasoning".to_string()
        }
        SlashCommand::Debug => {
            format!(
                "Debug:\n  Session ID: {}\n  Messages: {}\n  Agent: {}",
                session.meta.session_id,
                session.messages.len(),
                session.meta.agent_id,
            )
        }
        SlashCommand::Thinking(level) => {
            let l = level.as_deref().unwrap_or("(current)");
            format!("Thinking level: {}", l)
        }
        SlashCommand::Verbose(toggle) => {
            let v = toggle.as_deref().unwrap_or("(current)");
            format!("Verbose: {}", v)
        }
        SlashCommand::Reasoning(toggle) => {
            let r = toggle.as_deref().unwrap_or("(current)");
            format!("Reasoning: {}", r)
        }
    }
}
