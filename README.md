# RepoCtx — AI Context & Impact Engine for Codebases

## Overview

RepoCtx is a developer-first CLI and MCP server that turns a repository into **persistent, queryable knowledge** for AI coding agents and developers. Its goal is to give an agent the *right code plus the right understanding* for a task — without dumping the whole repo into the context window, and without re-deriving everything on every query.

**North star:** one call → one context bundle (markdown) with verified code snippets + meaning + impact, within a token budget. See [ADR-0006](./docs/adr/0006-grounded-knowledge-wiki.md).

Instead of treating a repository as a collection of files, RepoCtx maintains three coordinated layers:

1. **Deterministic Core** — a precise, code-derived graph of symbols, dependencies, flows, entry points and change-impact. This is *ground truth*: measured from the source, never guessed.
2. **Knowledge Layer (Repo Wiki)** — a persistent, compounding set of markdown pages (module/service/flow/concept) that explain *intent, conventions and gotchas*. Pages are **anchored to real symbols** and authored lazily by the host agent's model (via MCP sampling) — never re-derived from scratch.
3. **Context Assembly** — on a query, RepoCtx returns the relevant **wiki page + actual code snippets + impact set**, packed within a token budget. This is the "code in context" layer.

The deterministic core is what makes the wiki trustworthy: because RepoCtx knows the real call graph, it can **ground** page generation and **lint** pages against the code (flagging stale or contradictory claims). A plain LLM wiki cannot verify itself; RepoCtx can.

It acts as a bridge between:
- Large codebases
- AI coding agents (Claude Code, Cursor, Codex, etc.)
- Developers navigating and refactoring systems

---

## Use today (v0.1 — shipped)

These work **now**, with no LLM required:

| Command / tool | What you get |
|---|---|
| `repoctx build` | Deterministic graph: symbols, deps, flows, entrypoints → `.repoctx/*.json` |
| `repoctx impact <symbol>` | What breaks downstream (call graph + modules) |
| `repoctx flow <domain>` | End-to-end execution path across services |
| `repoctx context <symbol>` | Symbol metadata, neighbors, related components (JSON) |
| `repoctx build --watch` | Incremental rebuild on file changes |
| `repoctx workspace build` | Cross-repo linking (HTTP/gRPC/queue) |
| MCP `get_impact`, `get_flow`, `get_dependencies` | Same queries for Cursor / Claude Code agents |
| MCP `get_context` | Metadata bundle + optional MCP sampling enrichment |

### Quick start (3 steps)

```bash
# 1. Install (pick one)
cargo install repoctx-cli repoctx-mcp --locked
# or: npx repoctx build   (downloads binary)

# 2. Index your repo
cd your-project && repoctx build

# 3. Query before you edit
repoctx impact PaymentService
repoctx flow payment
```

### Cursor / Claude Code (MCP)

Add to your MCP config (`.cursor/mcp.json` or Claude Code settings):

```json
{
  "mcpServers": {
    "repoctx": {
      "command": "repoctx-mcp",
      "env": {
        "REPOCTX_ROOT": "${workspaceFolder}"
      }
    }
  }
}
```

Run `repoctx build` once per repo (or use `repoctx build --watch` in a terminal). The agent can then call `get_impact` / `get_flow` / `get_context` before modifying code.

---

## Coming in v0.2 (Knowledge Layer)

Planned — see [BACKLOG.md](./BACKLOG.md) P1-8 … P1-15:

| Feature | Deliverable |
|---|---|
| Context Assembly | `repoctx context X --format md --budget 6000 --task fix` → **one markdown file** with real code snippets + impact |
| Grounded Repo Wiki | `.repoctx/wiki/*.md` anchored to symbol IDs, compiled from the graph |
| Wiki lint | Stale/contradiction detection vs live graph (`repoctx wiki lint`) |
| Watch → wiki sync | `build --watch` queues stale pages for re-sync |
| MCP `get_wiki` | Fetch grounded wiki pages |

Until v0.2, `get_context` returns structured metadata (symbol, file:line, neighbors) — not full source snippets yet. Use `impact` + `flow` for the highest-value queries today.

---

## The Problem

Modern AI coding tools are powerful but fundamentally limited by context:

### 1. Context Window Limitations
Even large models struggle when:
- repositories exceed hundreds or thousands of files
- multiple layers of abstraction exist
- domain logic is scattered

### 2. Lack of Architectural Understanding
LLMs typically:
- understand local code snippets
- fail to reconstruct system-wide architecture
- hallucinate dependencies or flows

### 3. Poor Impact Awareness
Agents often:
- modify code without understanding side effects
- break unrelated features
- miss hidden dependencies

### 4. No Persistent Repository Memory
Every session is stateless:
- no durable understanding of the system
- repeated analysis cost
- inconsistent reasoning over time

---

## The Solution

RepoCtx introduces a **local intelligence layer** that continuously analyzes a repository and exposes both *structure* and *meaning* — plus the code itself, on demand.

It builds a persistent representation of:

- Architectural structure
- Domain concepts
- Execution flows
- Entry points
- Dependencies
- Symbol relationships
- Change impact maps
- **A grounded knowledge wiki** (markdown pages tied to real symbols)

---

## Core Idea

Two patterns dominate "codebase memory" today, and each has a hole:

- **RAG / pure retrieval** re-derives context on every query, struggles with code chunking, and has no persistent understanding.
- **LLM Wiki** (persistent markdown knowledge) compounds over time but **cannot verify itself** — it drifts from the code and hallucinates relationships.

RepoCtx combines them and closes both holes:

> "Here is a *deterministically verified* understanding of the repository, a *compounding wiki* grounded in that verification, and the *exact code* you need — within your token budget."

Instead of asking an AI to "read this repository and understand it", or trusting a wiki that may be stale, RepoCtx gives the agent:

- **verified structure** (the graph never guesses),
- **persistent meaning** (the wiki, grounded and lint-checked against the graph),
- **the right code** (snippets assembled per query, not the whole repo).

This reduces token usage, hallucinations and context-rebuilding cost, while increasing accuracy, consistency and speed of reasoning.

> **Out of scope:** a *wiki-only* tool (markdown without a verifying graph). RepoCtx's value is precisely the graph-grounded verification underneath the wiki.

---

## CLI Interface

### Initialize analysis

```bash
repoctx build
```

Generates:

```
.repoctx/
  architecture.json
  symbols.json
  flows.json
  dependencies.json
  entrypoints.json
  wiki/               # (v0.2) grounded knowledge pages
    index.md
    <page>.md
```

The JSON artifacts are produced deterministically with **no model required**.

---

### Knowledge wiki *(planned v0.2)*

```bash
repoctx wiki sync     # create/update pages for changed areas (lazy, via MCP host)
repoctx wiki lint     # flag stale, contradictory, or orphan pages against the graph
repoctx wiki show OrderService
```

`lint` is the differentiator: because the deterministic graph is ground truth, RepoCtx can detect when
a page claims a relationship the code no longer has.

---

### Query impact of changes

```bash
repoctx impact UserService
```

Output:
- modules affected
- downstream dependencies
- related tests
- potential risk zones

---

### Understand a flow

```bash
repoctx flow payment
```

Output:
- end-to-end execution path
- service interactions
- external systems involved

---

### Generate AI-ready context

**Today (v0.1):**

```bash
repoctx context PaymentService --json
```

Returns symbol metadata, file location, related components, and semantic neighbors.

**Planned (v0.2):**

```bash
repoctx context PaymentService --format md --budget 6000 --task fix
```

One markdown bundle within the token budget:

- relevant **wiki page** (intent, conventions, gotchas)
- **actual code snippets** (callers/callees, sliced from disk)
- **impact set** and related tests

This is the primary agent deliverable — one file, not four tools to orchestrate.

---

## Key Features

### 1. Deterministic Code Graph
Code-derived symbols, dependencies, calls, flows and entry points — measured, not guessed.

### 2. Impact Analysis Engine
Determines what breaks when a component changes.

### 3. Grounded Knowledge Wiki *(v0.2)*
A persistent, compounding markdown knowledge base, anchored to real symbols and lint-checked against
the graph so it can't silently drift from the code.

### 4. Context Assembly *(v0.2 — metadata today)*
Returns the right code snippets plus the right understanding for a task, packed within a token budget.
Today: symbol metadata via `context`; v0.2: markdown bundle with real snippets.

### 5. Repository Memory Layer
Maintains persistent structural *and* semantic understanding across sessions.

---

## Integration with AI Tools

RepoCtx is designed to be **agent-agnostic**.

### Claude Code / Claude CLI

Agents can call:

```bash
repoctx context <symbol>
repoctx impact <symbol>
repoctx flow <domain>
```

to retrieve precise context before modifying code.

---

### Cursor IDE

Cursor can integrate RepoCtx as a background context provider:
- enrich code suggestions with architectural awareness
- reduce incorrect refactors
- improve multi-file edits

---

### OpenAI Codex / Future CLI Agents

Any agent can use RepoCtx as a tool:

```bash
tools:
  - repoctx.context
  - repoctx.impact
  - repoctx.flow
```

This enables structured reasoning over large codebases.

---

### MCP (Model Context Protocol) Integration

RepoCtx exposes an MCP server:

```
repoctx-mcp
```

Available tools:
- **get_impact**, **get_flow**, **get_dependencies** — shipped
- **get_context** — shipped (metadata + optional sampling enrichment; full snippet bundle in v0.2)
- **get_wiki** — planned v0.2

This allows seamless integration with modern AI agents. Wiki authoring/enrichment runs through the
host agent's model via **MCP sampling** — RepoCtx bundles no LLM and holds no API keys.

---

## Design Principles

### 1. Deterministic First
Core analysis should be deterministic where possible.

### 2. AI-Augmented, Not AI-Dependent
AI enhances interpretation (wiki prose, names, summaries), but **structure is derived from code** and
the wiki is always **verifiable against the graph**. Remove the AI layer and you lose prose, never
correctness.

### 3. Local-First
All analysis runs locally to ensure:
- privacy
- speed
- reproducibility

### 4. Machine-Readable Outputs
All outputs must be:
- JSON-compatible
- stable schema
- versioned

---

## Why This Matters

The future of software development is:
- AI-assisted
- multi-agent
- context-heavy

But current systems lack:
- persistent understanding of codebases
- structured architectural memory
- reliable impact reasoning

RepoCtx fills this gap by becoming the **semantic layer between code and intelligence**.

---

## Long-Term Vision

RepoCtx aims to become:

> The standard context layer for all AI coding agents — a **verified, compounding memory** of a codebase.

In the same way Git became the standard for version control,
RepoCtx aims to become the standard for:

- code understanding
- AI context retrieval (code + knowledge, not just metadata)
- a self-verifying knowledge wiki grounded in the code
- architectural reasoning

---

## Success Criteria

The tool is successful if:

- developers use it daily before commits/PRs
- AI agents call it before modifying code
- it reduces debugging and refactoring errors
- it becomes part of standard dev workflow

---

## Development

### Prerequisites

- Rust 1.75+ (`rustup`)

### Build from source

```bash
git clone https://github.com/GabrieleRuggieri/repo-ctx.git
cd repo-ctx
cargo build --release
./target/release/repoctx build
```

### Install (prebuilt)

**Homebrew** (after tap is published):

```bash
brew tap GabrieleRuggieri/repoctx
brew install repoctx
```

**npm** (downloads native binary from GitHub Releases):

```bash
npx repoctx build
```

**Cargo** (builds from source):

```bash
cargo install repoctx-cli --locked
cargo install repoctx-mcp --locked
```

**GitHub Releases**: download the archive for your platform from [Releases](https://github.com/GabrieleRuggieri/repo-ctx/releases), or use the shell installer attached to each release.

See [packaging/README.md](./packaging/README.md) for maintainers cutting a new version.

### MCP server (AI agents)

```bash
# Dalla root del repo da analizzare (dopo `repoctx build`)
export REPOCTX_ROOT=.
cargo run --bin repoctx-mcp --release
```

Tools esposti: `get_context`, `get_impact`, `get_flow`, `get_dependencies`, `get_wiki`.

### Run tests & lint

```bash
cargo test --all
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

### Git hooks (rustfmt pre-commit)

Setup una tantum (formatta automaticamente prima di ogni commit):

```bash
./scripts/setup-git-hooks.sh
```

Il gate CI `cargo fmt --check` resta attivo come rete di sicurezza.

### Documentation map

| Document | Purpose |
|---|---|
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Stack, data model, API contracts (source of truth) |
| [CODEMAP.md](./CODEMAP.md) | Execution flow and crate graph |
| [PROGRESS.md](./PROGRESS.md) | Development log / completed milestones |
| [BACKLOG.md](./BACKLOG.md) | Open work prioritized P0–P2 |
| [RULES.md](./RULES.md) | Git, commit, testing, and code quality conventions |

### License

Apache-2.0 — see [LICENSE](./LICENSE).

---

## Conclusion

RepoCtx is not just another developer tool.

It is a **missing layer between codebases and AI reasoning systems**.

It transforms raw repositories into a verified graph, a compounding knowledge wiki grounded in that
graph, and on-demand code bundles — a queryable knowledge system that both humans and AI agents can
rely on consistently, without re-reading the whole repo or trusting an unverifiable wiki.
