# Mandala + FieldVault Steering

ANOMALY is steering toward a mandala-native and fieldvault-sealed architecture.

## Core Rule

JIMI does not primarily save agents as Markdown.

JIMI should save:

- agent identity as `Mandala`
- sealed private payloads as `FieldVault`
- markdown only as projection or export when useful

## Implications

### Agent Packaging

Agents become installable capsules with:

- identity
- execution policy
- memory policy
- skills
- databases or refs
- internal programs
- sacred shards

### Personality Slots

The runtime should support slot-based agent loading, where each slot is backed by a mandala capsule.

### Marketplace

The house should eventually support a marketplace where capsules can be:

- installed
- updated
- sold
- shared freely

### Security

Sensitive payloads and sacred shards should be sealed via `FieldVault` rather than left as ordinary files.
