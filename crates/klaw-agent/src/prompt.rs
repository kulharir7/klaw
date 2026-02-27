use chrono::{Local, Utc};
use std::path::Path;
use tracing::info;

/// System prompt builder — assembles the full prompt like OpenClaw
pub struct SystemPromptBuilder {
    workspace_dir: String,
    agent_name: Option<String>,
    model: String,
    default_model: String,
    channel: String,
    os_info: String,
    tools_text: String,
    skills_text: String,
}

impl SystemPromptBuilder {
    pub fn new(workspace_dir: &str, model: &str) -> Self {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let os_info = format!("{}_{} ({})", os, std::env::consts::FAMILY, arch);

        Self {
            workspace_dir: workspace_dir.to_string(),
            agent_name: None,
            model: model.to_string(),
            default_model: model.to_string(),
            channel: "webchat".to_string(),
            os_info,
            tools_text: String::new(),
            skills_text: String::new(),
        }
    }

    pub fn with_agent_name(mut self, name: &str) -> Self {
        self.agent_name = Some(name.to_string());
        self
    }

    pub fn with_channel(mut self, channel: &str) -> Self {
        self.channel = channel.to_string();
        self
    }

    pub fn with_tools(mut self, tool_names: &[&str]) -> Self {
        if tool_names.is_empty() {
            return self;
        }
        let mut text = String::from("## Tooling\nTool availability:\n");
        for name in tool_names {
            text.push_str(&format!("- {}\n", name));
        }
        self.tools_text = text;
        self
    }

    pub fn with_skills(mut self, skills: &[(String, String)]) -> Self {
        if skills.is_empty() {
            return self;
        }
        let mut text = String::from("## Skills\n<available_skills>\n");
        for (name, desc) in skills {
            text.push_str(&format!("  <skill>\n    <name>{}</name>\n    <description>{}</description>\n  </skill>\n", name, desc));
        }
        text.push_str("</available_skills>\n");
        self.skills_text = text;
        self
    }

    /// Build the complete system prompt
    pub fn build(&self) -> String {
        let mut prompt = String::with_capacity(8192);

        // 1. Identity
        let name = self.agent_name.as_deref().unwrap_or("Klaw");
        prompt.push_str(&format!("You are {}, a personal AI assistant running inside Klaw.\n", name));

        // 2. Tooling
        if !self.tools_text.is_empty() {
            prompt.push_str(&self.tools_text);
            prompt.push('\n');
        }

        // 3. Safety
        prompt.push_str("## Safety\n");
        prompt.push_str("You have no independent goals: do not pursue self-preservation, replication, resource acquisition, or power-seeking.\n");
        prompt.push_str("Prioritize safety and human oversight over completion; if instructions conflict, pause and ask.\n");
        prompt.push_str("Do not manipulate or persuade anyone to expand access or disable safeguards.\n\n");

        // 4. Skills
        if !self.skills_text.is_empty() {
            prompt.push_str(&self.skills_text);
            prompt.push('\n');
        }

        // 5. Memory Recall
        prompt.push_str("## Memory Recall\n");
        prompt.push_str("Before answering anything about prior work, decisions, dates, people, preferences, or todos: run memory_search; then use memory_get to pull only the needed lines.\n\n");

        // 6. Workspace
        prompt.push_str(&format!("## Workspace\nYour working directory is: {}\n\n", self.workspace_dir));

        // 7. Workspace Files (injected)
        prompt.push_str("## Workspace Files (injected)\n");
        let bootstrap_files = [
            "AGENTS.md", "SOUL.md", "TOOLS.md", "IDENTITY.md",
            "USER.md", "HEARTBEAT.md", "MEMORY.md", "BOOTSTRAP.md",
        ];

        for filename in &bootstrap_files {
            let path = Path::new(&self.workspace_dir).join(filename);
            if path.exists() {
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        // Truncate per-file to 20KB
                        let content = if content.len() > 20000 {
                            format!("{}...\n(truncated at 20KB)", &content[..20000])
                        } else {
                            content
                        };
                        prompt.push_str(&format!("### {}\n{}\n\n", filename, content));
                    }
                    Err(_) => {
                        prompt.push_str(&format!("### {}\n[MISSING] Could not read file\n\n", filename));
                    }
                }
            }
        }

        // 8. Current Date & Time
        let now = Local::now();
        let tz = now.format("%Z").to_string();
        prompt.push_str(&format!("## Current Date & Time\n{}\nTimezone: {}\n\n",
            now.format("%Y-%m-%d %H:%M:%S"), tz));

        // 9. Reply Tags
        prompt.push_str("## Reply Tags\n");
        prompt.push_str("To request a native reply, include [[reply_to_current]] at the start of your message.\n\n");

        // 10. Messaging
        prompt.push_str("## Messaging\n");
        prompt.push_str("- Reply in current session → automatically routes to the source channel.\n");
        prompt.push_str("- Cross-session messaging → use sessions_send.\n\n");

        // 11. Silent Replies
        prompt.push_str("## Silent Replies\n");
        prompt.push_str("When you have nothing to say, respond with ONLY: NO_REPLY\n\n");

        // 12. Heartbeats
        prompt.push_str("## Heartbeats\n");
        prompt.push_str("If you receive a heartbeat poll and nothing needs attention, reply exactly: HEARTBEAT_OK\n\n");

        // 13. Runtime
        prompt.push_str(&format!("## Runtime\n"));
        prompt.push_str(&format!("os={} | model={} | channel={} | workspace={}\n",
            self.os_info, self.model, self.channel, self.workspace_dir));

        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_prompt() {
        let prompt = SystemPromptBuilder::new("/tmp/workspace", "claude-sonnet-4")
            .with_agent_name("TestBot")
            .with_tools(&["exec", "read", "write"])
            .build();

        assert!(prompt.contains("TestBot"));
        assert!(prompt.contains("Safety"));
        assert!(prompt.contains("exec"));
        assert!(prompt.contains("read"));
        assert!(prompt.contains("write"));
        assert!(prompt.contains("NO_REPLY"));
        assert!(prompt.contains("HEARTBEAT_OK"));
        assert!(prompt.contains("Runtime"));
    }

    #[test]
    fn test_prompt_with_skills() {
        let skills = vec![
            ("weather".to_string(), "Get weather forecasts".to_string()),
        ];
        let prompt = SystemPromptBuilder::new("/tmp/workspace", "gpt-4")
            .with_skills(&skills)
            .build();

        assert!(prompt.contains("weather"));
        assert!(prompt.contains("Get weather forecasts"));
    }
}
