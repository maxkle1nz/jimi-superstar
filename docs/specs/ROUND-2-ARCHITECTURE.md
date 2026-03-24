# Round 2 Architecture

Round 2 of RETROBUILDER changes the constitutional center of the house.

## Before

- markdown and generic artifacts were treated as the main durable semantic medium
- skills were the main conceptual installable

## After

- events remain motion truth
- mandalas become agent-state truth
- fieldvault artifacts become sealed private truth
- capsules become the primary installable
- skills become sub-assets inside capsules

## Required Runtime Additions

- `house.mandalas`
- `house.capsules`
- `runtime.fieldvault`
- slot-aware session and routing logic

## Required Surface Additions

- mandala inspector
- slot manager
- capsule marketplace
- shard unlock broker

## Required Storage Additions

- mandala manifests
- slot bindings
- fieldvault artifact registry
- capsule ownership records
