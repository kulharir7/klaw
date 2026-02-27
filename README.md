# Klaw 🦀

**Self-hosted AI Gateway in Rust.** Connect chat apps to AI agents.

Klaw is a complete reimplementation of [OpenClaw](https://docs.openclaw.ai/) — built from scratch in Rust for speed, safety, and simplicity.

## What is Klaw?

Klaw sits between your chat apps (Telegram, Discord, WhatsApp, etc.) and AI models (OpenAI, Anthropic, Google, etc.). It gives your AI agent tools, memory, and the ability to talk across platforms.

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Telegram    │     │             │     │  OpenAI     │
│  Discord     │────▶│    Klaw     │────▶│  Anthropic  │
│  WhatsApp    │◀────│   Gateway   │◀────│  Google     │
│  WebChat     │     │             │     │  Ollama     │
│  Slack  ...  │     └─────────────┘     │  33+ more   │
└─────────────┘           │              └─────────────┘
                    ┌─────┴─────┐
                    │  25 Tools │
                    │  Sessions │
                    │  Memory   │
                    │  Cron     │
                    └───────────┘
```

## Features

### 🤖 33 LLM Providers
OpenAI, Anthropic, Google Gemini, Vertex AI, OpenRouter, Groq, Cerebras, Together, xAI, Mistral, GitHub Copilot, Hugging Face, Cloudflare, NVIDIA, Ollama, vLLM, LM Studio, and 16 more. Use `provider/model` format (e.g., `anthropic/claude-sonnet-4-20250514`).

### 💬 22 Chat Channels
Telegram (full), Discord, Slack, WhatsApp, Signal, IRC, Google Chat, BlueBubbles (iMessage), MS Teams, Matrix, and 12 more via plugin system.

### 🔧 25 Agent Tools
| Category | Tools |
|----------|-------|
| **Runtime** | `exec`, `process` |
| **Files** | `read`, `write`, `edit`, `apply_patch` |
| **Web** | `web_search` (Brave), `web_fetch` |
| **Memory** | `memory_search`, `memory_get` |
| **Sessions** | `sessions_list`, `sessions_history`, `sessions_send`, `sessions_spawn`, `session_status`, `agents_list` |
| **UI** | `browser`, `canvas` |
| **Automation** | `cron`, `gateway` |
| **Messaging** | `message` |
| **Nodes** | `nodes` |
| **Media** | `image`, `tts` |

### 🔐 Tool Policy System
- **Profiles:** `minimal`, `coding`, `messaging`, `full`
- **Groups:** `group:runtime`, `group:fs`, `group:web`, `group:memory`, `group:sessions`, `group:ui`, `group:automation`, `group:messaging`, `group:nodes`
- **Allow/Deny lists** with glob matching (deny wins)
- **Per-provider restrictions** via `tools.byProvider`

### 🔑 OAuth
5 OAuth providers: Google Antigravity, Google Gemini CLI, Qwen Portal, OpenAI Codex, Anthropic Max. Device code + authorization code flows with token persistence.

### 🏗️ Architecture
- **WebSocket protocol** with connect handshake, auth tokens, device pairing
- **Session store** with disk persistence (JSON index + JSONL transcripts)
- **DM scoping** (main, per-peer, per-channel-peer, per-account-channel-peer)
- **Presence tracking** (online/offline/typing)
- **Idempotency cache** (5-min TTL dedupe)
- **PID lock file** (prevents double-start)
- **System prompt builder** (13 sections — identity, tools, safety, skills, memory, workspace, bootstrap files, datetime, reply tags, messaging, silent replies, heartbeats, runtime)

## Quick Start

### Install

```bash
# Clone and build
git clone https://github.com/kulharir7/klaw.git
cd klaw
cargo build --release
```

### Configure

Create `~/.klaw/klaw.json`:

```json5
{
  gateway: {
    port: 19789,
    host: "127.0.0.1",
  },
  agents: {
    defaults: {
      model: "anthropic/claude-sonnet-4-20250514",
      api_key: "sk-ant-...",
    },
  },
}
```

### Run

```bash
# Start the gateway
klaw gateway start

# Open WebChat
# http://localhost:19789

# Or test from CLI
klaw test -m "Hello, what can you do?"

# Check status
klaw status
klaw doctor
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `klaw gateway start` | Start the gateway server |
| `klaw gateway stop` | Stop the gateway |
| `klaw gateway status` | Check if gateway is running |
| `klaw gateway restart` | Restart the gateway |
| `klaw health` | Health check |
| `klaw status` | Full status overview |
| `klaw agent` | Run agent interactively |
| `klaw message send` | Send a message |
| `klaw config get/set` | View/edit configuration |
| `klaw models` | List all 33 providers |
| `klaw auth login/logout/status` | OAuth management |
| `klaw test` | Quick test message |
| `klaw doctor` | Diagnose issues |
| `klaw version` | Show version |

## Project Structure

```
klaw/
├── crates/
│   ├── klaw-core/       # Config, sessions, types, errors
│   ├── klaw-gateway/    # Axum HTTP+WS server
│   ├── klaw-agent/      # LLM providers, agent loop, prompt builder
│   ├── klaw-channels/   # Chat platform integrations
│   ├── klaw-tools/      # 25 agent tools
│   └── klaw-cli/        # CLI binary
└── webchat/             # WebChat UI (dark theme)
```

## Configuration

Klaw uses JSON5 config at `~/.klaw/klaw.json`. Supports comments and trailing commas.

```json5
{
  // Gateway settings
  gateway: {
    port: 19789,
    host: "127.0.0.1",
    token: "optional-auth-token",  // or set KLAW_GATEWAY_TOKEN env var
  },

  // Agent defaults
  agents: {
    defaults: {
      model: "anthropic/claude-sonnet-4-20250514",
      provider: "anthropic",  // auto-detected from model if omitted
      api_key: "sk-...",
      workspace: "~/.klaw/workspace",
    },
  },

  // Session management
  session: {
    dm_scope: "main",  // main | per-peer | per-channel-peer | per-account-channel-peer
  },

  // Tool policies
  tools: {
    profile: "full",          // minimal | coding | messaging | full
    allow: ["group:fs", "group:web"],
    deny: ["browser"],
    byProvider: {
      "openai/gpt-4o": { profile: "coding" },
    },
  },

  // Model aliases
  models: {
    aliases: {
      "sonnet": "anthropic/claude-sonnet-4-20250514",
      "opus": "anthropic/claude-opus-4-0-20250514",
    },
  },

  // Channel configs
  channels: {
    telegram: { bot_token: "123:ABC..." },
    discord: { bot_token: "..." },
    webchat: { enabled: true },
  },
}
```

## Tech Stack

- **Language:** Rust (edition 2024)
- **HTTP/WS:** [axum](https://github.com/tokio-rs/axum)
- **Async:** [tokio](https://tokio.rs/)
- **CLI:** [clap](https://github.com/clap-rs/clap)
- **Serialization:** serde + JSON5
- **HTTP Client:** reqwest
- **Logging:** tracing

## Roadmap

- [x] Core gateway + WebSocket protocol
- [x] 33 LLM providers + OAuth
- [x] 25 agent tools + policy system
- [x] 22 chat channels
- [x] Session persistence + DM scoping
- [x] System prompt builder
- [x] WebChat UI
- [ ] Telegram full integration (wire up)
- [ ] Model failover chains
- [ ] Semantic memory search
- [ ] Browser automation (Playwright/CDP)
- [ ] Cron job scheduler
- [ ] Desktop app (Tauri)

## License

MIT

---

Built with 🦀 by [Ravindra Kumar](https://github.com/kulharir7)
