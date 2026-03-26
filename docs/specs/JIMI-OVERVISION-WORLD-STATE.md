# JIMI OVERVISION World State

Status: active
Scope: canonical donor integration for host-state and computer-state context inside `ANOMALY`

---
Protocol: L1GHT/1.0
Node: JIMIOvervisionWorldState
State: active
Glyph: ⌬
---

## Purpose

This document defines how the `OVERVISION` concept should enter `ANOMALY`.

It does not replace the house.

It contributes a missing substrate:

- world state
- host state
- computer context
- real machine observability

## Donor Thesis

Recovered `OVERVISION` thesis:

- the whole computer becomes a navigable and editable brain through `m1nd`
- the filesystem stops being a dead hierarchy and becomes a persistent, incremental graph
- code is only one region of the machine
- tools are executable projections of a world model

Recovered `OVERVISION` primitives:

1. layered indexing
2. federated or sharded graph
3. incremental ingest from filesystem events
4. hybrid storage
5. active-region caching

## JIMI Thesis

JIMI already owns:

- runtime cognition
- provider routing
- capsule memory
- mandala state
- fieldvault sealing
- control plane
- security and policy
- autonomy

Therefore:

- `OVERVISION` should not become the whole architecture
- `OVERVISION` should become the world-state substrate inside the architecture

## Canonical Placement

JIMI's memory and truth layers should now be understood as:

1. `session memory`
2. `user memory`
3. `task memory`
4. `capsule memory`
5. `sealed memory`
6. `world state`

Ownership:

- `events` own motion truth
- `Mandala` owns agent-state truth
- `FieldVault` owns sealed private truth
- `roomanizer` owns organism continuity
- `m1nd` owns structural truth
- `m1nd OVERVISION` owns host-state and world-state truth

## Core Rule

The goal is not total context injection.

The goal is total context availability.

That means:

- the machine is pre-modeled
- the graph is incrementally maintained
- the agent queries slices on demand
- only the relevant slice enters the `ContextPacket`

## World State Scope

The `world state` plane may include:

- filesystem regions
- running processes
- app state
- windows and tabs
- services and ports
- logs
- local databases
- device sensors
- network edges
- background jobs
- workspace topology

Not every category must be active at first.

The plane is canonical even if implementation is phased.

## Architectural Components

### `world.index`

Layered metadata-first indexing.

Examples:

- path
- file hash
- mime
- size
- timestamps
- protected vs public class

### `world.ingest`

Incremental updates driven by:

- filesystem events
- process changes
- app/window telemetry
- runtime health events

### `world.graph`

A graph of host entities and their relations.

Examples:

- file uses file
- app opened file
- service writes log
- project owns artifact
- task depends on workspace state

### `world.cache`

Hot active-region cache for:

- current workspace
- current room
- active apps
- recent paths

### `world.query`

Bounded retrieval surface.

Examples:

- what changed
- what is running
- what is connected to this project
- what files are related to this artifact
- what app owns this window

### `world.snapshot`

A compressed, policy-aware slice of current machine truth.

This is what should feed the execution context.

## Context Assembly

`ContextPacket` should become capable of including a `world_state_slice`.

That slice should be:

- task-scoped
- policy-bounded
- room-aware where relevant
- revocable
- observable in the cockpit

Provider lanes should receive:

- only the world-state slice needed for the task
- not the raw machine

## Security

World-state access must obey house policy.

Rules:

- no implicit omniscience
- no raw unrestricted host dump by default
- capabilities remain typed and revocable
- sensitive locations can be sealed or excluded
- access is auditable

`OVERVISION` strengthens `truth before fluency`.

It must not weaken `capability security`.

## Relationship To Existing JIMI Systems

### With `Capsule Memory`

`Capsule Memory` tracks what happened in interaction and work.

`World State` tracks what is true in the machine.

They are complementary.

### With `Mandala`

`MandalaMemoryPolicy` should influence:

- what world-state scopes are preferred
- what world-state scopes are denied
- how much world-state can enter a packet

### With `FieldVault`

Sensitive world-state slices may be:

- sealed
- redacted
- referenced indirectly

### With `Control Plane`

The control plane should expose:

- world-state health
- indexed regions
- active-region cache
- excluded zones
- snapshot previews
- access policy

## Implementation Order

1. add `world state` as a named plane in architecture and UIX
2. define `WorldStateSlice` in the runtime grammar
3. expose world-state status in the control plane
4. start with workspace and process snapshots
5. add incremental ingest
6. add graph relations and active-region cache
7. assemble bounded world-state slices into `ContextPacket`

## Anti-Goals

Do not:

- replace `Mandala + FieldVault + Capsule Memory`
- dump the whole machine into prompts
- make MCP the semantic center
- treat `OVERVISION` as a second orchestrator

## Canonical Sentence

Inside ANOMALY, `OVERVISION` is the donor that turns the real computer into an on-demand world-state substrate, while the house remains sovereign over memory, policy, execution, and context assembly.
