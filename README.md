# JIMI SUPERSTAR

JIMI SUPERSTAR is the sovereign multi-engine agent house.

It is designed to host many engines, import many skill families, expose one coherent operator cockpit, and keep memory, policy, truth, and orchestration owned by the house rather than by any single provider.

## House Thesis

JIMI SUPERSTAR is not:

- a wrapper around one model provider
- a chat shell with plugins
- a fork of another agent platform

JIMI SUPERSTAR is:

- a Rust-first kernel
- an event-backed runtime
- a universal capability house
- a multi-truth system
- a cockpit-driven operator surface

## Stage 1 Focus

The first implementation slice builds the minimal sovereign kernel:

- canonical events
- durable session truth
- WebSocket event streaming
- one real engine lane
- one minimal conversation surface

## Repository Layout

- `crates/jimi-kernel/`: house kernel crate
- `apps/jimi-cockpit/`: operator cockpit app placeholder
- `docs/architecture/`: house architecture overview
- `docs/specs/`: frozen architecture contracts and build sequence

## Immediate Build Order

1. `house.events`
2. `memory.durable`
3. `house.sessions`
4. `api.ws`
5. `providers.codex`
6. minimal conversation surface

## Status

This repository is intentionally private and currently in constitutional foundation stage.
