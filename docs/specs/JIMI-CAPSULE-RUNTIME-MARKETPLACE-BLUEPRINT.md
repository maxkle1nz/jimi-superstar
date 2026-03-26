# JIMI Capsule Runtime + Marketplace Shell

Frozen next-phase blueprint for the first platform expansion after the operational core.

This phase turns ANOMALY from a sovereign operator house into a sovereign capsule platform.

## Phase Thesis

The operational core is now alive:

- memory works
- autonomy works
- provider rails work
- world-state slices work
- cockpit governance works

The next phase is not "more of the same."

It is:

- packaging
- installation
- exchange
- runtime isolation
- creator-facing distribution

JIMI should become the house where external agent capsules can be installed, activated, trusted, priced, audited, and shared.

## Constitutional Goal

JIMI should be able to host many agents that are built as `Mandala FieldVault Capsules`, each carrying their own:

- identity
- memory rules
- skills
- local data
- internal programs
- sacred shards
- provider affinities
- runtime policies
- trust posture

The house remains sovereign.

External capsules do not become second orchestrators.
They become installable runtime citizens under house policy.

## Core Outcomes

At the end of this phase, JIMI should support:

1. capsule package format
2. capsule import/export/install/uninstall
3. capsule verification and trust classification
4. slot activation for multiple personalities
5. marketplace shell in the cockpit
6. install-time policy review
7. isolated runtime bindings per capsule
8. sealed shard handling through FieldVault

## Runtime Thesis

The installable unit is not a loose skill pack.

The installable unit is a `Capsule Package`.

The capsule package contains:

- `mandala.manifest.json`
- `capsule.contract.json`
- `skills/`
- `programs/`
- `data/`
- `shards/`
- `assets/`
- `exports/`
- `signature / provenance metadata`

The capsule may reference sealed `.fld` artifacts rather than shipping everything in plaintext.

## Main Modules

### `capsule.packaging`

Responsible for:

- package layout
- manifest validation
- versioning
- import/export
- deterministic package digest

### `capsule.installer`

Responsible for:

- install flow
- dependency checks
- compatibility checks
- trust checks
- slot-binding suggestion
- rollback on failed install

### `capsule.registry`

Responsible for:

- installed capsules
- source provenance
- local status
- version and upgrade tracking
- dependency visibility

### `capsule.trust`

Responsible for:

- local/verified/unverified/internal classifications
- digest/signature checks
- source origin
- risk flags
- privilege warnings

### `capsule.runtime`

Responsible for:

- loading capsule programs
- activating capsule skills
- binding capsule data
- surfacing capsule capabilities to the house
- isolating side effects by policy

### `capsule.marketplace`

Responsible for:

- searchable catalog surface
- install candidates
- authorship and provenance
- pricing/license metadata
- local publish flow later

### `capsule.fieldvault`

Responsible for:

- importing sealed shards
- binding shard access to capsule + slot + trust policy
- portable vs machine-bound artifacts
- previewing shard requirements before activation

## Package Model

The canonical package object should include:

- `capsule_id`
- `mandala_id`
- `version`
- `display_name`
- `creator`
- `source_origin`
- `package_digest`
- `trust_level`
- `required_caps`
- `optional_caps`
- `provider_affinity`
- `slot_recommendation`
- `fieldvault_requirements`
- `compatibility_notes`

## Install Flow

1. select capsule package
2. parse and validate manifest
3. compute digest
4. classify trust level
5. inspect required capabilities
6. inspect required shards / fieldvault artifacts
7. preview slot impact
8. approve install
9. install capsule into registry
10. optionally bind to slot
11. optionally activate

## Activation Flow

1. resolve capsule from slot
2. validate trust and policy
3. unlock required shards
4. load capsule runtime bindings
5. mount capsule capabilities into the house
6. switch active personality
7. emit activation events

## Import / Export Flow

### Export

1. select installed capsule
2. package manifest + contract + assets
3. include or reference fieldvault shards according to policy
4. seal/export package
5. produce digest and provenance summary

### Import

1. ingest package
2. validate structure
3. validate digest / signature if present
4. preview policy impact
5. install under trust classification

## Security Model

Capsule install is never blind.

Every capsule must be evaluated on:

- trust level
- capability request surface
- filesystem scope
- network scope
- provider scope
- shard requirements
- runtime side effects

House rule:

- a capsule can extend the house
- a capsule cannot override house sovereignty

## Cockpit Surfaces

The next phase adds or upgrades these surfaces:

### `Capsule Registry`

- installed capsules
- version
- trust
- source
- slot bindings

### `Capsule Installer`

- import package
- preview impact
- approve install
- rollback failed install

### `Marketplace Shell`

- discover capsules
- inspect metadata
- compare trust/provenance
- install candidates

### `Slot Console`

- activate/deactivate capsules
- compare personalities
- see shard requirements
- swap active agent body

### `Capsule Trust Surface`

- trust class
- required capabilities
- provider affinities
- privileged warnings

## Events

This phase should add events like:

- `capsule_package_detected`
- `capsule_install_previewed`
- `capsule_install_started`
- `capsule_install_completed`
- `capsule_install_failed`
- `capsule_activated`
- `capsule_deactivated`
- `capsule_trust_classified`
- `capsule_export_completed`
- `capsule_marketplace_listed`

## Persistence

The runtime should gain durable tables for:

- package records
- source provenance
- trust decisions
- install history
- export history
- slot activation history

## Donors

This phase is grounded by:

- `Mandala + FieldVault`
- `roomanizer capsule thinking`
- `JIMI control plane`
- existing slot and artifact runtime

It should not regress toward:

- markdown-first saves
- plugin bundles without policy
- skill-only packaging

## Phase Gate

This phase is complete when:

1. a real capsule package can be imported
2. install preview is visible in cockpit
3. trust classification is visible and durable
4. a capsule can bind to a slot
5. a slot can activate a different installed personality
6. fieldvault requirements are surfaced before activation
7. the marketplace shell lists at least local/installable capsule entries

## Immediate Build Order

1. package record + registry
2. install preview + trust classification
3. cockpit registry / installer surface
4. slot activation flow for installed capsules
5. local marketplace shell
6. export/import hardening
