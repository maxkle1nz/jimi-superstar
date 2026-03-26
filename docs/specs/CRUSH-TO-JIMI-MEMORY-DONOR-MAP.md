# CRUSH to JIMI Memory Donor Map

This document records which memory ideas and mechanisms from `crush` should be imported into `ANOMALY`, which ones should be adapted, and which ones should remain outside the house.

It exists to prevent vague influence and replace it with explicit donor mapping.

## Verdict

`crush` is a strong donor for:

- memory persistence
- room-scoped recall
- transcript distillation
- retrieval ladders

`crush` is not the canonical memory model of JIMI.

JIMI remains grounded in:

- `Mandala` as agent-state truth
- `FieldVault` as sealed private truth
- `MemoryCapsule` as the live memory unit
- `ContextPacket` as the execution memory payload

## Import Now

These should be transplanted early.

### 1. Room-scoped durable store

Source donor:

- `engram_store.go`

Why:

- SQLite WAL and room-aware indexing are already proven there
- JIMI needs room-scoped memory for multi-session continuity

Target in JIMI:

- `memory.durable`
- `memory.namespace`

Planned shape:

- add `room_id` to capsule persistence
- eventually add `house_id`
- keep queryable local store separate from `FieldVault`

### 2. Recall ladder

Source donor:

- `agent.go` fallback order

Why:

- the recall order is disciplined and useful
- it prevents jumping straight from session failure into global noise

Target in JIMI:

- `memory.query_layer`
- `ContextPacket assembly`

Planned shape:

- session
- lineage
- room
- global
- then `m1nd`
- then special summary/bridge recall

### 3. Transcript distiller worker

Source donor:

- `transcript_distiller.go`

Why:

- worker coalescing, cancellation, and stats are already solved cleanly
- JIMI needs exactly this pattern for post-turn distillation

Target in JIMI:

- `memory.distiller`
- `memory.orchestrator`

Planned shape:

- distill after turn completion
- support queue coalescing
- expose runtime stats in cockpit

## Adapt Later

These are valuable but should be translated into the JIMI house language.

### 4. Engram taxonomy

Source donor:

- `conversation/transcript`
- `conversation/episode`
- `conversation/directive`

Why:

- the granularity is strong
- it improves later recall quality

Adaptation rule:

- do not import `Engram` as the canonical save format
- import these as capsule kinds or capsule tags

Target in JIMI:

- `MemoryCapsule.kind`
- `SummaryCheckpoint classification`

Planned future expansion:

- `conversation/decision`
- `conversation/promise`

### 5. FTS5 search layer

Source donor:

- `engram_store.go`

Why:

- fast local search is useful
- good fallback when semantic ranking is unavailable

Adaptation rule:

- FTS is one retrieval layer, not the whole brain

Target in JIMI:

- `memory.query_layer`

Used alongside:

- relevance scoring
- bridges
- summaries
- future embeddings
- `m1nd`

### 6. Runtime status observability

Source donor:

- `L1GHTStatus`

Why:

- health and worker stats matter operationally

Adaptation rule:

- render as house runtime health, not just agent-local status

Target in JIMI:

- `Runtime Observatory`
- `Memory Console`

## Do Not Import Literally

These should influence JIMI conceptually but not be copied as-is.

### 7. Engram as canonical truth object

Reason:

- JIMI already has a stronger constitutional center
- importing `Engram` as the main truth object would weaken `Mandala + FieldVault + Capsule Memory`

### 8. Markdown or companion text as canonical save

Reason:

- text exports are useful
- canonical saves in JIMI remain structured and house-native

Acceptable role:

- export
- compatibility
- audit companion

Not acceptable role:

- primary semantic persistence

### 9. Agent-monolith memory ownership

Reason:

- JIMI should keep memory modular

Preferred JIMI structure:

- capture
- distill
- classify
- promote
- seal
- recall

## First Planned Transplants

The next transplant order should be:

1. room-scoped memory namespace
2. transcript distiller worker
3. capsule kind taxonomy from transcript / episode / directive
4. recall ladder
5. local FTS layer

## House Rule

When JIMI absorbs a donor from `crush`, the donor must be translated into the house grammar.

That means:

- `Engram` ideas become capsule-memory concepts
- transcript blocks become capsule kinds or summary inputs
- store improvements become durable-memory improvements

The donor may strengthen the house.
The donor may not replace the house.
