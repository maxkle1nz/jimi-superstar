---
description: M1ND-first code intelligence rules — enforced on every boot and every operation
---

# M1ND-First Operating Rules

## Critical Rule

**You MUST use m1nd MCP tools BEFORE falling back to grep, glob, find, or file reads.**

m1nd is a local code graph engine available via MCP. It ingests code into a structural graph and exposes 64 tools for navigation, analysis, and editing. It is faster, more contextual, and more structural than raw text search.

## Tool Replacement Map

| Traditional Tool | M1ND Replacement | Notes |
|-----------------|------------------|-------|
| `grep` / `rg` | `m1nd.search(mode="literal")` | Finds exact strings with graph context. Use `auto_ingest=true` for paths outside current roots |
| `glob` / `find` | `m1nd.activate` | Finds related code by meaning, not filename |
| `cat` / `Read` | `m1nd.surgical_context_v2` | Returns source + ALL connected files in one call, with heuristic_summary |
| Manual dep check | `m1nd.impact` | Shows blast radius before edits |
| File write | `m1nd.apply` | Writes code AND auto-updates the graph |
| Multi-file write | `m1nd.apply_batch(verify=true)` | Write N files + 5-layer post-write verification (SAFE/RISKY/BROKEN) |
| "What tool?" | `m1nd.help()` | When unsure which tool to use |

## Use Case Patterns

- **Bug hunt:** `hypothesize` → `missing` → `flow_simulate` → `trace`
- **Pre-deploy gate:** `antibody_scan` → `validate_plan` → `epidemic`
- **Architecture audit:** `layers` → `layer_inspect` → `counterfactual`
- **Onboarding:** `activate` → `layers` → `perspective.start` → `perspective.follow`
- **Cross-domain search:** `ingest(adapter="memory", mode="merge")` → `activate`
- **Safe multi-file edit:** `surgical_context_v2` → `apply_batch(verify=true)`

## Boot Checklist

// turbo-all
1. Check if m1nd graph is ingested for the current workspace
2. If not, run `m1nd.ingest` on the workspace root
3. Use m1nd tools for ALL code navigation before falling back to traditional tools

## Configuration

- **Binary:** `/Users/cosmophonix/SISTEMA/m1nd/target/release/m1nd-mcp`
- **MCP Config:** `~/.gemini/antigravity/mcp_config.json`
- **Transport:** stdio (JSON-RPC)
- **Current root:** `/Users/cosmophonix/jimi-superstar`
