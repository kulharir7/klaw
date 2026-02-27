# RULES.md — Klaw Development Rules (PERMANENT)

> These rules MUST be followed in EVERY session. No exceptions.
> Read this file FIRST before writing any code.

## 🎯 Rule #1: OpenClaw is the Blueprint

**Before writing ANY code for a module, FIRST study OpenClaw's actual implementation.**

```
OpenClaw source: C:\Users\kulha\AppData\Roaming\npm\node_modules\openclaw\dist\plugin-sdk\
```

### How to study:
1. Read ALL `.d.ts` files in the module directory (they show the full type signatures)
2. Read the `.js` files for actual implementation logic
3. Count their files, functions, types — that's your TARGET
4. List every feature/edge case they handle
5. THEN write Klaw code that matches that depth

### Module mapping (OpenClaw → Klaw):
| OpenClaw Module | Files | Klaw Crate | Our Files |
|----------------|-------|------------|-----------|
| `telegram/` | 45 | klaw-channels | 1 ❌ |
| `discord/` | 70 | klaw-channels | 1 ❌ |
| `slack/` | 52 | klaw-channels | 1 ❌ |
| `agents/` | 293 | klaw-agent | 12 ❌ |
| `config/` | 104 | klaw-core | 1 ❌ |
| `gateway/` | 46 | klaw-gateway | 5 ❌ |
| `sessions/` | 7 | klaw-core | 1 ❌ |
| `memory/` | 46 | klaw-tools | 2 ❌ |
| `browser/` | 65 | klaw-tools | 1 ❌ |
| `process/` | 12 | klaw-tools | 1 ❌ |
| `auto-reply/` | 149 | klaw-agent | 0 ❌ |
| `channels/` | 81 | klaw-channels | 1 ❌ |
| `cron/` | 19 | klaw-gateway | 1 ❌ |
| `infra/` | 136 | klaw-core | 0 ❌ |
| `web/` | 42 | klaw-gateway | 1 ❌ |
| `security/` | 6 | klaw-core | 1 ❌ |
| `providers/` | 3 | klaw-agent | 4 ✅ |
| `plugins/` | 23 | — | 0 ❌ |
| `routing/` | 5 | klaw-gateway | 0 ❌ |
| `media/` | 16 | klaw-tools | 0 ❌ |
| `tts/` | 2 | klaw-tools | 1 ❌ |
| `shared/` | 20 | klaw-core | 0 ❌ |
| `utils/` | 18 | klaw-core | 0 ❌ |
| `logging/` | 13 | klaw-core | 0 ❌ |

**OpenClaw total: 867 files, 33.6 MB**
**Klaw current: 62 files, 360 KB**
**Gap: ~14x fewer files, ~93x less code**

---

## 🎯 Rule #2: Never Write Stubs

**Every function must do real work.** No more:
- `return "Not yet implemented"`
- `// TODO: implement`
- Placeholder returns
- Empty match arms with just a string

If you can't implement it fully, DON'T create the file yet. Better to have 10 real files than 50 fake ones.

---

## 🎯 Rule #3: One Module at a Time, Full Depth

**WRONG approach (what we did):**
```
Session 1: Write 50 skeleton files → "42% done!" (actually 5% done)
```

**RIGHT approach (what to do now):**
```
Session 1: Study OpenClaw's telegram/ (45 files) → Write klaw telegram (10-15 files) → DEEP
Session 2: Study OpenClaw's agents/ → Write klaw agent properly → DEEP
Session 3: Study OpenClaw's gateway/ → Write klaw gateway properly → DEEP
```

### Per-module checklist:
```
□ Read ALL OpenClaw .d.ts files for this module
□ List every function, type, and edge case
□ Write Klaw implementation matching each function
□ Handle errors properly (not just anyhow::bail)
□ Add retry logic where OpenClaw has it
□ Add logging at same verbosity level
□ Handle all edge cases (empty input, network errors, rate limits, etc.)
□ Write tests (at least 3-5 per module)
□ Compare line count: if ours is <30% of theirs, we're missing stuff
```

---

## 🎯 Rule #4: Quality Metrics Before Claiming Done

A module is "done" ONLY when:
1. **Feature parity:** Every function in OpenClaw's `.d.ts` has a Klaw equivalent
2. **Error handling:** Proper Result types, retry logic, graceful degradation
3. **Tests:** Minimum 3 unit tests per module
4. **Edge cases:** Empty input, timeout, rate limit, auth failure, network error
5. **Code size:** At least 30% of OpenClaw's line count for that module
6. **Actually works:** Tested end-to-end, not just compiles

---

## 🎯 Rule #5: Study Before Code

**Every session, before writing code:**
1. Read `RULES.md` (this file)
2. Read `memory/YYYY-MM-DD.md` for yesterday's progress
3. Pick ONE module to work on
4. Run: `Get-ChildItem "$env:APPDATA\npm\node_modules\openclaw\dist\plugin-sdk\<module>" -File | Select Name, Length`
5. Read the `.d.ts` files to understand the full API surface
6. THEN start coding

---

## 🎯 Rule #6: Module Priority Order

Work on modules in this order (impact × feasibility):

### Priority 1 — Core Engine (must be solid)
1. `agents/` → `klaw-agent/` (agent loop, streaming, compaction, auto-reply)
2. `gateway/` → `klaw-gateway/` (WS protocol, routing, events)
3. `config/` → `klaw-core/config.rs` (full config loading, validation, defaults)
4. `sessions/` → `klaw-core/session.rs` (persistence, reset, maintenance)

### Priority 2 — Channels (user-facing)
5. `telegram/` → `klaw-channels/telegram/` (SPLIT into multiple files)
6. `discord/` → `klaw-channels/discord/`
7. `slack/` → `klaw-channels/slack/`
8. `channels/` → `klaw-channels/` (shared channel infra)

### Priority 3 — Tools (agent capabilities)
9. `process/` → `klaw-tools/process.rs`
10. `browser/` → `klaw-tools/browser/`
11. `memory/` → `klaw-tools/memory/`
12. `media/` → `klaw-tools/media/`

### Priority 4 — Infrastructure
13. `infra/` → `klaw-core/infra/` (retry, rate limit, queue, etc.)
14. `auto-reply/` → `klaw-agent/auto-reply/`
15. `cron/` → `klaw-gateway/cron/`
16. `web/` → Control UI
17. `security/` → `klaw-core/security/`
18. `routing/` → `klaw-gateway/routing/`

---

## 🎯 Rule #7: File Structure Matches OpenClaw

**DON'T put everything in one file.** If OpenClaw has 45 files for Telegram, we should have at least 10-15 files organized by concern:

```
klaw-channels/src/telegram/
├── mod.rs          — Module entry, re-exports
├── bot.rs          — Bot creation, webhook setup
├── send.rs         — Send text, media, stickers, polls
├── receive.rs      — Parse updates, build message context
├── commands.rs     — Native bot commands (/start, /help, etc.)
├── groups.rs       — Group access, mention detection, topic support
├── buttons.rs      — Inline keyboards, callback queries
├── reactions.rs    — Message reactions, status reactions
├── media.rs        — Media download, upload, file handling
├── streaming.rs    — Draft stream editing (progressive response)
├── format.rs       — Markdown/HTML formatting for Telegram
├── types.rs        — Telegram-specific types
├── helpers.rs      — Utility functions
└── webhook.rs      — Webhook mode support
```

---

## 🎯 Rule #8: Reference Commands

```powershell
# List OpenClaw module files
Get-ChildItem "$env:APPDATA\npm\node_modules\openclaw\dist\plugin-sdk\<module>" -File | Select Name, Length

# Read OpenClaw type definitions
Get-Content "$env:APPDATA\npm\node_modules\openclaw\dist\plugin-sdk\<module>\<file>.d.ts"

# Read OpenClaw implementation
Get-Content "$env:APPDATA\npm\node_modules\openclaw\dist\plugin-sdk\<module>\<file>.js"

# Compare file counts
$oc = (Get-ChildItem "$env:APPDATA\npm\node_modules\openclaw\dist\plugin-sdk\<module>" -Recurse -File).Count
$kl = (Get-ChildItem "C:\Users\kulha\projects\klaw\crates\<crate>\src\<module>" -Recurse -File -ErrorAction SilentlyContinue).Count
echo "OpenClaw: $oc files | Klaw: $kl files | Gap: $($oc - $kl)"
```

---

## 🎯 Rule #9: Commit Messages Reference OpenClaw

Every commit should reference what OpenClaw feature was studied:

```
feat(telegram): add streaming draft edits (ref: openclaw telegram/draft-stream)
feat(agent): add proper compaction with memory flush (ref: openclaw agents/compaction)
```

---

## 🎯 Rule #10: Progress Tracking

After each module rewrite, update `references/GAP_ANALYSIS.md`:
- Change status from ❌/🔸 to ✅
- Add "Depth: X/Y functions" metric
- Add "Files: X (OpenClaw has Y)" metric
- Add "Tests: X" metric

---

## Current Reality Check

| What We Have | What It Should Be |
|-------------|-------------------|
| 62 files, 360 KB | ~300+ files, 3+ MB |
| 1 file per module | 5-50 files per module |
| Basic happy path | Full error handling + retries |
| "Compiles" | "Works in production" |
| 17 tests | 200+ tests |
| Stubs everywhere | Real implementations |

**We're at ~5% of OpenClaw's depth. The structure is right, the depth is wrong.**

---

*Last updated: 2026-02-27*
*These rules are permanent. Follow them every session.*
