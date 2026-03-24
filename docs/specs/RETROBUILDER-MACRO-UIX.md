# RETROBUILDER Macro UIX

This document defines the macro communication surface for the active project.

If `RETROBUILDER-CHAT-UIX.md` is the micro loop ritual, this document is the panoramic ritual.

The macro surface exists so the user can ask for project-wide truth at any moment without needing to reconstruct it from recent messages.

## Purpose

The macro surface answers:

1. what is the whole project state
2. what has already been completed
3. what is actively being built now
4. what still remains
5. what questions the user can ask next

## Required Macro Blocks

Every macro projection should include:

- global `RETROBUILDER` status
- global `L1GHT` status
- subsystem bars
- done / doing / next / later
- menu of callable questions

## Macro Footer Rule

At the end of meaningful runs, the system should emit:

- micro slice status
- macro project status

The user should never have to ask "where are we globally?" after a major milestone.

## Canonical Macro Sections

### 1. Global Axes

```text
RETROBUILDER  [████████░░] 87%  R4 live loops materializing
L1GHT         [████████░░] 85%  L4 live surfaces
PROJECT       [███████░░░] 78%  sovereign house operational, ecosystem ahead
```

### 2. Subsystem Bars

Recommended starting set:

- `RUNTIME`
- `MEMORY`
- `COCKPIT`
- `PROVIDERS`
- `CAPSULES`
- `MARKETPLACE`
- `SECURITY`

### 3. Work Buckets

- `DONE`
- `DOING`
- `NEXT`
- `LATER`

### 4. Macro Menu

The system should expose a callable menu in chat and, when possible, in the cockpit.

Canonical menu:

```text
MENU
[1] what was completed
[2] what is being built now
[3] what remains
[4] show full panorama
[5] show subsystem status
[6] show latest milestones
[7] show risks and blocks
[8] show recommended next step
```

Natural language aliases are preferred in normal conversation.

## Data Sources

The macro surface may be derived from:

- frozen specs
- implementation sequence
- live inventory
- milestone commits
- runtime state
- subsystem heuristics

It must not pretend false precision when the project is still moving.

## Precision Rule

Percentages are a communication tool, not fake accounting.

They should be:

- grounded
- stable enough to be useful
- updated only when the project meaningfully changes phase

## Relationship To L1GHT

`L1GHT` compresses architectural truth.

`RETROBUILDER Macro UIX` compresses project progress truth.

## Relationship To JIMI

JIMI SUPERSTAR should answer both the micro question and the macro question by default.

Micro:
- what changed in this run

Macro:
- where does the whole house stand now
