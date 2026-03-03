# Klaw vs OpenClaw — Full Gap Analysis
> Generated: 2026-02-27 | Source: OpenClaw docs (llms.txt, 200+ pages)
> Updated: 2026-03-03 | Reflects current Klaw implementation

## Legend
- ✅ Done (working in Klaw)
- 🔸 Partial (stub or incomplete)
- ❌ Missing (not started)

---

## 1. CORE GATEWAY (`klaw-gateway`)

| Feature | Status | Notes |
|---------|--------|-------|
| HTTP server (axum) | ✅ | Health, WebChat, Canvas/A2UI endpoints |
| WebSocket protocol | ✅ | Connect handshake, hello-ok |
| Gateway auth token | ✅ | Config + KLAW_GATEWAY_TOKEN env |
| PID lock file | ✅ | gateway.lock |
| Device pairing | ✅ | PairingStore, approve/reject |
| Presence tracking | ✅ | Online/offline, connected_at |
| Idempotency cache | ✅ | 5-min TTL |
| Event: tick | ✅ | 15s keepalive |
| Event: presence | ✅ | Connect/disconnect |
| Event: agent lifecycle | ✅ | Start/end/error with runId |
| Canvas endpoint | ✅ | /__klaw__/canvas/ |
| A2UI endpoint | ✅ | /__klaw__/a2ui/ |
| OpenAI Chat Completions API | ✅ | `/v1/chat/completions` HTTP endpoint |
| Tools Invoke HTTP API | ✅ | `/v1/tools/invoke` endpoint |
| Bridge protocol | ❌ | For remote gateway access |
| Bonjour/mDNS discovery | ❌ | Local network auto-discover |
| Multiple gateways | ❌ | Federation support |
| Tailscale integration | ❌ | Remote access via Tailscale |
| Gateway-owned pairing | ❌ | Challenge-nonce signing |
| Trusted proxy auth | ❌ | Proxy authentication |
| Secrets management | ✅ | Encrypted secrets store with master key |
| Sandbox (Docker) | ❌ | Container isolation for agents |

## 2. CONFIG SYSTEM (`klaw-core`)

| Feature | Status | Notes |
|---------|--------|-------|
| JSON5 config | ✅ | ~/.klaw/klaw.json |
| Gateway config | ✅ | port, host, token, verbose |
| Agent defaults | ✅ | model, provider, api_key, base_url, workspace |
| Session config | ✅ | dm_scope, main_key |
| Tools config | ✅ | allow, deny, profile, by_provider |
| Channels config | ✅ | telegram, discord, whatsapp, webchat |
| Model aliases | ✅ | models.aliases HashMap |
| Model failover config | 🔸 | Sub-agent building (failover, api_keys, retry_count) |
| model as object {primary, fallbacks} | ❌ | OpenClaw supports string OR object form |
| imageModel config | ❌ | Primary + fallbacks for vision |
| Per-agent list | 🔸 | AgentEntry has id/model/workspace, missing: identity, sandbox, tools, heartbeat, params, groupChat, subagents |
| Bindings (multi-agent routing) | ✅ | BindingResolver routes channels→agents |
| Channel model overrides (modelByChannel) | ❌ | Per-channel model pinning |
| Channel defaults | ❌ | groupPolicy, heartbeat settings |
| Per-channel full config | 🔸 | Basic fields only, missing: streaming, actions, groups, custom commands, retry, historyLimit, etc. |
| Session reset (daily/idle) | ✅ | is_idle(), is_expired(), cleanup methods |
| Session maintenance | 🔸 | prune, rotate - partial |
| Session identity links | ✅ | IdentityStore for cross-channel identity |
| Session thread bindings | ❌ | Thread-bound sessions |
| Session send policy | ❌ | deny/allow rules |
| Compaction | ✅ | Chunked summarization implemented |
| Context pruning | ✅ | Prune old tool results from context |
| Block streaming config | ❌ | Chunk-based message delivery |
| Typing indicators config | ❌ | never/instant/thinking/message |
| Agent identity (name, emoji, avatar) | ❌ | Per-agent branding |
| Agent heartbeat config | ❌ | every, model, session, target, prompt |
| Commands config | ❌ | native, text, bash, config, restart, allowFrom |
| Sandbox config | ❌ | Docker, workspace access, browser, prune |
| CLI backends | ❌ | Fallback text-only CLI tools |
| Multi-account channels | ❌ | Multiple accounts per channel |
| Webhook config | ❌ | Inbound webhooks |
| Auth profiles | ✅ | AuthProfileManager with key rotation |
| userTimezone | ❌ | Timezone config |
| bootstrapMaxChars | ✅ | 20000 default |
| bootstrapTotalMaxChars | ✅ | 150000 default |
| maxConcurrent | ❌ | Parallel agent runs limit |
| timeoutSeconds | ❌ | Agent run timeout |
| mediaMaxMb | ❌ | Media size limits |
| contextTokens | ❌ | Max context window |

## 3. AGENT (`klaw-agent`)

| Feature | Status | Notes |
|---------|--------|-------|
| Agent loop (message→LLM→tools→response) | ✅ | Max 10 rounds |
| System prompt builder (13 sections) | ✅ | All sections implemented |
| OpenAI provider | ✅ | Streaming + non-streaming |
| Anthropic provider | ✅ | Native API |
| Provider registry (33) | ✅ | All OpenAI-compatible providers |
| OAuth (5 providers) | ✅ | Device code + auth code flows |
| Model failover chains | 🔸 | Sub-agent building now |
| Auth profile rotation | ❌ | Round-robin, cooldowns, session stickiness |
| Cooldown (exponential backoff) | ❌ | 1m→5m→25m→1h cap |
| Billing disables | ❌ | Detect "insufficient credits" |
| Compaction (safeguard mode) | ❌ | Chunked summarization |
| Memory flush before compaction | ❌ | Auto-store memories |
| Context pruning | ❌ | Prune old tool results |
| Streaming (partial/block/progress) | ✅ | WebSocket streaming via StreamChunk |
| Human delay | ❌ | Randomized reply delay |
| Thinking levels | ❌ | low/medium/high thinking |
| Image model routing | ❌ | Route images to vision model |
| CLI backend fallback | ❌ | Text-only fallback |
| Tool loop detection | ❌ | Block repeat/pingpong loops |
| maxConcurrent enforcement | ❌ | Semaphore for parallel runs |
| Agent timeout | ❌ | Kill long-running agents |

## 4. TOOLS (`klaw-tools`)

| Tool | Status | Notes |
|------|--------|-------|
| exec | ✅ | Shell commands, timeout, truncate |
| process | ✅ | Background sessions (list/poll/log/kill) |
| read | ✅ | File read with offset/limit |
| write | ✅ | File write, auto-create dirs |
| edit | ✅ | Find-and-replace |
| apply_patch | ✅ | Multi-hunk patches |
| web_search | ✅ | Brave Search API |
| web_fetch | ✅ | HTML→text, SSRF protection |
| memory_search | ✅ | Keyword search (not semantic yet) |
| memory_get | ✅ | Snippet read with from/lines |
| image | ✅ | Vision API (OpenAI/Anthropic) |
| tts | ✅ | OpenAI TTS with cross-platform playback |
| message | ✅ | Multi-channel messaging |
| cron | ✅ | Scheduled jobs with gateway integration |
| gateway | ✅ | Gateway control |
| sessions_list | ✅ | List sessions |
| sessions_history | ✅ | Get session history |
| sessions_send | ✅ | Send message to session |
| sessions_spawn | ✅ | Spawn sub-agent |
| session_status | ✅ | Session stats |
| agents_list | ✅ | List agents |
| browser | ✅ | Chrome DevTools Protocol |
| canvas | ✅ | Canvas operations |
| nodes | ✅ | Node management |
| Tool policy (profiles/groups/allow/deny) | ✅ | 4 profiles, 10 groups |
| Tool byProvider filtering | ✅ | Config support |
| Tool loop detection | ❌ | genericRepeat, knownPollNoProgress, pingPong |
| exec: pty support | ❌ | Pseudo-terminal |
| exec: elevated mode | ❌ | Host execution |
| exec: host/node targeting | ❌ | Run on gateway/node |
| exec: security modes | ❌ | deny/allowlist/full |
| exec: approvals | ❌ | Approval workflow |
| process: send-keys | ❌ | TTY key sending |
| process: paste | ❌ | Bracketed paste |
| browser: Playwright/CDP | ❌ | Real browser control |
| browser: profiles | ❌ | Multi-profile management |
| browser: Chrome extension relay | ❌ | Attach to existing Chrome |
| Semantic memory search | ❌ | Embedding-based search |
| Slash commands | ✅ | /help, /status, /model, /new, /reset, /usage, /version, /agents |
| Plugins system | ❌ | Register custom tools |
| Lobster (workflow runtime) | ❌ | Typed pipelines with approvals |
| LLM Task tool | ❌ | JSON-only LLM step |
| ACP Agents | ❌ | Agent Communication Protocol |

## 5. CHANNELS (`klaw-channels`)

| Channel | Status | Notes |
|---------|--------|-------|
| WebChat | ✅ | Dark UI, WS-based |
| Telegram | ✅ | Long polling, wired to gateway |
| Discord | ✅ | Gateway WebSocket, wired to gateway |
| Slack | ✅ | Socket Mode, wired to gateway |
| WhatsApp | 🔸 | Sidecar stub |
| Signal | 🔸 | Sidecar stub |
| IRC | 🔸 | Stub |
| Google Chat | 🔸 | Webhook stub |
| BlueBubbles | 🔸 | REST API stub |
| iMessage | 🔸 | Legacy macOS stub |
| 12 plugin channels | 🔸 | Macro-generated stubs |
| DM policies (pairing/allowlist/open/disabled) | ❌ | Access control |
| Group policies | ❌ | allowlist/open/disabled |
| Mention gating | ❌ | requireMention, patterns |
| Text chunking | ❌ | Per-channel limits |
| Streaming (partial/block/progress) | ❌ | Live message updates |
| Reactions | ❌ | Cross-channel reactions |
| Multi-account per channel | ❌ | Multiple bots/numbers |
| Channel history limits | ❌ | Per-channel/per-DM |
| Self-chat mode | ❌ | Own number in allowFrom |
| Custom commands (Telegram) | ❌ | Bot menu entries |
| Thread support | ❌ | Thread isolation |
| Voice (Discord) | ❌ | Voice channel conversations |

## 6. CLI (`klaw-cli`)

| Command | Status | Notes |
|---------|--------|-------|
| gateway start/stop/status/restart | ✅ | Working |
| health | ✅ | Health check |
| status | ✅ | Full status |
| agent | ✅ | Interactive agent |
| message send | ✅ | Send message |
| config get/set | ✅ | Config management |
| models | ✅ | List providers |
| auth login/logout/status | ✅ | OAuth management |
| test | ✅ | Quick test |
| doctor | ✅ | Diagnostics |
| version | ✅ | Version info |
| configure (interactive wizard) | ✅ | Guided setup |
| onboard | ✅ | First-time setup wizard |
| setup | ✅ | Quick provider setup |
| agents add/list | ✅ | Multi-agent management |
| sessions list/inspect/reset/send | ✅ | Session management |
| cron list/add/remove | ❌ | Cron management |
| browser (control) | ❌ | Browser management |
| channels login/status/probe | ❌ | Channel management |
| devices list/approve/reject | ❌ | Device management |
| memory search/get/set | ❌ | Memory management |
| plugins install/remove/list | ❌ | Plugin management |
| skills list/install | ❌ | Skill management |
| logs | ❌ | Log viewing |
| dashboard | ❌ | Web dashboard |
| tui | ❌ | Terminal UI |
| pairing | ❌ | Pairing management |
| secrets set/get/list | ❌ | Secrets management |
| webhooks | ❌ | Webhook management |
| hooks | ❌ | Hooks management |
| completion | ❌ | Shell completion |
| reset | ✅ | Factory reset with --force |
| memory search/get/set | ✅ | Memory management |
| uninstall | ❌ | Cleanup |
| update | ❌ | Self-update |
| dns | ❌ | DNS management |
| qr | ❌ | QR code for pairing |
| sandbox | ❌ | Sandbox management |
| security | ❌ | Security audit |
| system | ❌ | System info |
| voicecall | ❌ | Voice call management |
| node | ❌ | Node management |

## 7. WEB UI

| Feature | Status | Notes |
|---------|--------|-------|
| WebChat (basic) | ✅ | Dark theme, WS-based |
| Control UI | ❌ | Web-based admin panel |
| Dashboard | ❌ | Stats, usage, sessions |
| TUI | ❌ | Terminal UI |

## 8. CONCEPTS (Runtime Behaviors)

| Feature | Status | Notes |
|---------|--------|-------|
| Agent workspace files | ✅ | AGENTS.md, SOUL.md, USER.md, etc. |
| Bootstrap file injection | ✅ | 8 files, 20KB cap |
| Memory (MEMORY.md + daily) | ✅ | Read/write |
| Session persistence | ✅ | JSON index + JSONL |
| DM scoping (4 modes) | ✅ | main/per-peer/per-channel-peer/per-account |
| Reply tags | ✅ | [[reply_to_current]] in system prompt |
| Silent replies (NO_REPLY) | ✅ | System prompt guidance |
| Heartbeat (HEARTBEAT_OK) | ✅ | System prompt guidance |
| Runtime info in prompt | ✅ | Agent, host, repo, os, model, channel |
| Compaction | ❌ | Summarize long sessions |
| Context pruning | ❌ | Prune old tool outputs |
| Session reset (daily/idle) | ❌ | Auto-reset |
| Usage tracking | 🔸 | Token counts only, no cost |
| Prompt caching | ❌ | Anthropic/Google cache |
| Typing indicators | ❌ | Platform-specific |
| Streaming + chunking | ❌ | Live response delivery |
| Markdown formatting | ❌ | Per-channel formatting |
| Timezone handling | ❌ | Per-user timezone |
| Presence (online/typing) | 🔸 | Connect/disconnect only |
| Command queue | ❌ | Ordered execution |
| Retry policy | ❌ | Per-channel retries |

## 9. AUTOMATION

| Feature | Status | Notes |
|---------|--------|-------|
| Cron jobs | ✅ | Cron scheduler + agent processing |
| Heartbeat polling | ✅ | Configurable via heartbeat_every |
| Webhooks (inbound) | 🔸 | Webhook endpoint exists |
| Hooks (event triggers) | ❌ | On-event automation |
| Polls (periodic checks) | ❌ | Scheduled polling |
| Gmail PubSub | ❌ | Email notifications |
| Auth monitoring | ❌ | Token expiry alerts |

## 10. PLATFORMS

| Feature | Status | Notes |
|---------|--------|-------|
| macOS app | ❌ | Native Swift app |
| iOS app | ❌ | Mobile companion |
| Android app | ❌ | Mobile companion |
| Linux app | ❌ | Native or Flatpak |
| Windows (WSL2) | ❌ | WSL support |
| Desktop (Tauri/Electron) | ❌ | Planned for later |

## 11. NODES

| Feature | Status | Notes |
|---------|--------|-------|
| Node pairing | 🔸 | PairingStore exists |
| Camera capture | ❌ | Front/back camera |
| Screen recording | ❌ | Screen capture |
| Location | ❌ | GPS coordinates |
| Audio/voice notes | ❌ | Audio processing |
| Talk mode | ❌ | Real-time voice |
| Voice wake | ❌ | Wake word detection |

## 12. SECURITY

| Feature | Status | Notes |
|---------|--------|-------|
| Gateway auth token | ✅ | Config + env var |
| DM pairing | ❌ | One-time codes |
| Secrets management | ❌ | Encrypted store |
| Docker sandbox | ❌ | Container isolation |
| Exec approvals | ❌ | Approval workflow |
| Elevated mode | ❌ | Controlled escalation |
| Formal verification | ❌ | Security models |

---

## SUMMARY COUNTS

| Category | Done | Partial | Missing | Total |
|----------|------|---------|---------|-------|
| Core Gateway | 12 | 0 | 10 | 22 |
| Config | 11 | 3 | 24 | 38 |
| Agent | 6 | 1 | 14 | 21 |
| Tools | 13 | 11 | 14 | 38 |
| Channels | 1 | 12 | 12 | 25 |
| CLI | 11 | 0 | 25 | 36 |
| Web UI | 1 | 0 | 3 | 4 |
| Concepts | 10 | 2 | 10 | 22 |
| Automation | 0 | 1 | 6 | 7 |
| Platforms | 0 | 0 | 6 | 6 |
| Nodes | 0 | 1 | 6 | 7 |
| Security | 1 | 0 | 6 | 7 |
| **TOTAL** | **66** | **31** | **136** | **233** |

### Completion: **~42%** (66 done + 31 partial out of 233 features)

---

## PRIORITY ORDER (Recommended)

### Phase A — Make it Work End-to-End (next)
1. Wire Telegram channel to gateway (most impactful)
2. Convert stub tools to real (sub-agent doing this now)
3. Model failover (sub-agent doing this now)
4. Streaming support (partial at least)
5. Slash commands (/model, /new, /reset)

### Phase B — Production Ready
6. DM policies + pairing codes
7. Session reset (daily/idle)
8. Compaction
9. Usage tracking with cost
10. Typing indicators
11. More CLI commands (sessions, cron, agents)

### Phase C — Full Feature Parity
12. Multi-agent routing + bindings
13. Docker sandbox
14. Auth profile rotation + cooldowns
15. Browser tool (CDP)
16. Webhook support
17. Cron scheduler
18. Plugin system

### Phase D — Platform
19. Control UI / Dashboard
20. TUI
21. Desktop app (Tauri)
22. Node support (camera, screen, location)
