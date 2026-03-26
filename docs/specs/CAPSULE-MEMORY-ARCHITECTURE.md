# JIMI Capsule Memory Architecture

Status: active
Scope: canonical memory architecture for `ANOMALY`

---
Protocol: L1GHT/1.0
Node: JIMICapsuleMemoryArchitecture
State: active
Glyph: ⍌
---

## Purpose

This document defines how memory should work inside `ANOMALY`.

It formalizes the capsule-based memory vision as a runtime architecture rather than leaving it as an intuition.

It covers:

- memory units
- temporal bands
- summary generation
- memory graphing
- query and retrieval
- promotion and compaction
- context packet assembly
- integration with `Mandala`, `FieldVault`, `m1nd`, and `roomanizer`

## Thesis

JIMI should not treat context as a linear transcript that gets blindly truncated.

JIMI should treat memory as a living capsule stream that is:

- temporal
- compressed
- queryable
- relevance-weighted
- resumable
- partially sealed

The provider should not receive "chat history".

The provider should receive a `ContextPacket` assembled from the correct memory layers.

## Core Concepts

### `MemoryCapsule`

The canonical unit of conversational and operational memory.

Each capsule may contain:

- message content
- role
- turn id
- session id
- lane id
- tool outputs
- intent summary
- tags
- relevance score
- confidence score
- semantic links
- summary references
- artifact references
- privacy class
- timestamps

Rule:

every meaningful live interaction should be representable as one or more capsules.

### `SummaryCheckpoint`

A compressed memory artifact derived from a band of capsules.

It should contain:

- summary id
- source band
- source capsule range
- semantic digest
- decisions retained
- unresolved items
- extracted entities/topics
- confidence
- generated at

### `MemoryBridge`

A relation connecting non-adjacent memory regions.

Examples:

- same topic
- same project
- same unresolved blocker
- same operator preference
- same tool failure pattern

### `ContextPacket`

The assembled memory payload sent into execution.

It may include:

- active mandala state
- hot capsules
- warm summary
- archive summary fragments
- graph bridges
- world-state slices
- `m1nd` retrievals
- `roomanizer` memory retrievals
- released `FieldVault` shards if allowed

## Temporal Bands

The capsule stream is partitioned into three main bands.

### 1. `hot`

Default window:

- last 5 capsules

Properties:

- highest fidelity
- immediately provider-eligible
- no aggressive compression
- reflects the current interaction frame

Primary use:

- direct context for the next turn

### 2. `warm`

Default window:

- capsules 5 through 10 behind the hot edge

Properties:

- lightly compressed
- still narratively coherent
- quickly recoverable

Primary use:

- near-past context recovery

### 3. `archive`

Default window:

- 10+ capsules back

Properties:

- highly compressed
- query-first, not prompt-first
- progressively synthesized

Primary use:

- long-range recall
- semantic search
- resume after gap

Rule:

the exact numerical boundaries can evolve, but the three-band model is canonical.

## Memory Planes

The capsule architecture sits across multiple memory planes.

### `motion truth`

Owned by:

- `events`

Purpose:

- append-only replay
- observability
- evidence

### `agent-state truth`

Owned by:

- `Mandala`

Purpose:

- identity
- stable memory
- active snapshot
- memory policy

### `sealed private truth`

Owned by:

- `FieldVault`

Purpose:

- sacred shards
- private summaries
- sensitive memory bundles
- operator-private memory

### `organism memory`

Owned by:

- `roomanizer`

Purpose:

- cortex
- checkpoints
- pulses
- room-scoped continuity

### `structural truth`

Owned by:

- `m1nd`

Purpose:

- graph retrieval
- host and project relations

### `world-state truth`

Owned by:

- `m1nd OVERVISION`

Purpose:

- host state
- filesystem state
- process and app state
- machine reality slices for on-demand context
- relationship reasoning
- impact and why-chains
- non-linear query

## Runtime Modules

The following runtime modules should be added formally.

### `memory.capsules`

Owns:

- capsule creation
- capsule indexing
- temporal band assignment
- capsule metadata

Inputs:

- events
- messages
- tool outputs
- turn results

Exports:

- capsule append api
- capsule lookup api
- band queries

### `memory.summary_engine`

Owns:

- hot summaries
- warm summaries
- archive summaries
- summary checkpoints

Inputs:

- `memory.capsules`
- `memory.relevance`

Exports:

- summary generation api
- summary checkpoint retrieval

### `memory.relevance`

Owns:

- relevance score
- confidence score
- decay rules
- priority rules

Inputs:

- recency
- user signals
- retrieval hits
- unresolved blockers
- topic recurrence

Exports:

- weighted memory ranking

### `memory.graph`

Owns:

- semantic bridges
- topic relations
- summary-to-capsule links
- memory topology

Inputs:

- capsules
- summaries
- `m1nd` retrievals

Exports:

- graph neighborhood query
- related-memory query

### `memory.compaction`

Owns:

- band transitions
- promotion to summary
- projection updates
- archive compression

Exports:

- compaction jobs
- compaction audit

### `memory.promotion`

Owns:

- rules for moving memory into `MandalaStableMemory`
- rules for updating `MandalaActiveSnapshot`
- rules for sealing items into `FieldVault`

### `memory.query_layer`

Owns:

- hybrid retrieval during chat
- context assembly inputs
- recency + graph + semantic + summary fusion

Exports:

- ranked memory candidates
- memory evidence packets

## Compaction Model

Compaction in JIMI must be intentional and multi-stage.

### Stage A: hot preservation

The hottest capsules remain mostly raw.

Allowed operations:

- tagging
- scoring
- lightweight normalization

### Stage B: warm synthesis

Once capsules move out of `hot`, the system creates a warm summary.

Warm summaries should preserve:

- active goals
- important claims
- decisions
- unresolved items
- entities and references

### Stage C: archive synthesis

Older capsules are progressively merged into archive summaries.

Archive summaries should preserve:

- long-range topic continuity
- operator preferences discovered
- recurring blockers
- important artifacts
- memory bridges

Rule:

raw memory is not deleted just because it was summarized.
It becomes less directly provider-visible but remains queryable.

## Promotion Rules

The system should support three promotions.

### Promotion 1: to `MandalaActiveSnapshot`

When:

- the item is operationally current
- it changes the next action
- it changes the local task frame

Examples:

- current goal
- blockers
- next actions
- live decisions

### Promotion 2: to `MandalaStableMemory`

When:

- the item is durable across sessions
- it reflects preference, rule, or persistent truth

Examples:

- operator style preferences
- recurring workflow preferences
- learned rules
- durable project assumptions

### Promotion 3: to `FieldVault`

When:

- the item is private
- sensitive
- sacred
- commercially protected

Examples:

- proprietary capsules
- sensitive project notes
- sacred shards
- high-trust memory bundles

## Relevance, Decay, Confidence

Each capsule and summary should carry at least:

- `relevance_score`
- `confidence_level`
- `last_touched_at`
- `decay_class`

### Relevance

Relevance should increase with:

- recency
- repeated reuse
- direct operator emphasis
- unresolved status
- bridge count
- retrieval success

### Decay

Decay must be non-linear.

Time alone should not erase meaning.

Low-value memories decay faster than high-value recurring memories.

### Confidence

Confidence should reflect:

- directness of evidence
- number of confirming sources
- freshness
- synthesis depth

## Re-synthesis Triggers

The system should trigger re-synthesis when:

- a topic reappears strongly
- a prior summary is contradicted
- multiple bridges converge on the same archive zone
- the operator resumes after a gap
- memory confidence drops below threshold

## Context Assembly

The canonical execution input is `ContextPacket`.

### Assembly Order

1. `MandalaActiveSnapshot`
2. hot capsules
3. warm summary
4. archive summary fragments
5. graph bridges
6. `m1nd` retrievals
7. `roomanizer` memory retrievals
8. released `FieldVault` shards if permitted

### Assembly Rules

- do not dump full transcript by default
- prioritize relevance over raw recency when needed
- preserve provenance
- preserve privacy class
- degrade gracefully when a memory plane is unavailable

## Storage Model

The architecture implies the following new durable families.

- `memory_capsules`
- `summary_checkpoints`
- `memory_bridges`
- `context_packets`
- `memory_scores`
- `memory_promotions`
- `memory_compactions`

The current `SQLite WAL` baseline remains correct.

Blob-backed storage should be used for large summaries or sealed memory bundles when useful.

## Integration With Existing JIMI

### What already fits now

- `events` already provide motion truth
- `turns` and `dispatches` already provide capsule grouping boundaries
- `MandalaStableMemory` already exists
- `MandalaActiveSnapshot` already exists
- `FieldVaultArtifact` already exists
- `provider lanes` already exist
- `m1nd` is the natural structural query layer
- `roomanizer` is the natural organism memory layer

### What still needs implementation

- actual `MemoryCapsule` type
- summary engine
- relevance engine
- graph bridge storage
- context packet assembler
- promotion engine
- resynthesis jobs

## UI And Operator Surfaces

The cockpit should eventually expose:

- live capsule stream
- hot/warm/archive views
- summary checkpoint inspector
- bridge graph inspector
- memory confidence indicators
- manual promotion controls
- shard unlock surface

## Canonical Outcome

When this architecture is complete, JIMI should behave like a living memory organism.

Not:

- a long transcript
- a blind truncation system
- a static vector memory

But:

- a dynamic capsule stream
- a layered memory system
- a graph-aware recall engine
- a summary-driven context assembler
- a sovereign memory house
