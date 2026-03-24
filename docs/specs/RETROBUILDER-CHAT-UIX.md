# RETROBUILDER Chat UIX

This document defines the human-friendly chat communication surface for the active build method.

Inside JIMI SUPERSTAR, RETROBUILDER is not only a build doctrine. It is also a visible chat ritual.

The system should report progress in a way that is:

- warm
- legible
- emotionally grounding
- operationally precise
- readable in terminals and chat panes

## Purpose

The user should always be able to answer five questions quickly:

1. where are we
2. what just changed
3. what is running now
4. how far along are we
5. what comes next

## Communication Principles

- status must feel alive, not bureaucratic
- the house should sound human-friendly without losing precision
- progress should be visible at both system level and slice level
- every meaningful update should include both `L1GHT` and `RETROBUILDER` position
- terminal-safe formatting should degrade gracefully in plain text

## Chat Surface

Every meaningful progress update should use this general shape:

1. house badge
2. emotional signal
3. current phase bars
4. active slice status
5. next move

## House Badge

Recommended prefix:

`⌜rmnzr⌟ ⌜✦ JIMI SUPERSTAR⌟`

This badge should remain stable across updates so the house feels continuous.

## Emotional Signal

Use compact human-friendly indicators to reduce coldness and improve scanability.

Allowed examples:

- `alive`
- `grounded`
- `locked in`
- `clean`
- `green`
- `watching`
- `deepening`

Optional emoticon layer:

- `:)`
- `:D`
- `^_^`
- `o_o`
- `>_>`
- `<_<`
- `*`

Avoid spammy emoji floods. Use one or two emotional markers at most.

## Mandatory Progress Axes

Every substantial update should report:

- `RETROBUILDER phase`
- `L1GHT layer`
- `active implementation slice`
- `validation state`

## Canonical RETROBUILDER Phases

- `R1` idea field grounded
- `R2` constitutional contracts frozen
- `R3` sovereign center scaffolded
- `R4` live loops materializing
- `R5` ecosystem expansion
- `R6` operational refinement

## Canonical L1GHT Layers

- `L1` vision
- `L2` contracts
- `L3` runtime core
- `L4` live surfaces
- `L5` ecosystem and scale

## Progress Bar Standard

Use monospace bars in chat.

Primary style:

```text
RETROBUILDER  [████████░░] 80%  R4 live loops materializing
L1GHT         [████████░░] 80%  L4 live surfaces
MEMORY        [███████░░░] 70%  capsule orchestration entering provider flow
```

Secondary lightweight style:

```text
R: [#######---] 70%
L: [########--] 80%
M: [######----] 60%
```

## ANSI Terminal Style

When ANSI is supported, prefer:

- bold house badge
- cyan for house identity
- green for validated progress
- yellow for active work
- magenta for memory/context systems
- red only for blocks or regressions

Recommended palette:

```text
\x1b[1;36m⌜rmnzr⌟ ⌜✦ JIMI SUPERSTAR⌟\x1b[0m
\x1b[1;32mRETROBUILDER  [████████░░] 80%\x1b[0m
\x1b[1;34mL1GHT         [████████░░] 80%\x1b[0m
\x1b[1;35mMEMORY        [███████░░░] 70%\x1b[0m
\x1b[1;33mACTIVE SLICE  provider context packet\x1b[0m
```

If ANSI is not supported, fall back to plain monospace text with the same order.

## Update Templates

### 1. Short Loop Update

Use while work is in progress.

```text
⌜rmnzr⌟ ⌜✦ JIMI SUPERSTAR⌟  ^_^

RETROBUILDER  [████████░░] 80%  R4 live loops materializing
L1GHT         [████████░░] 80%  L4 live surfaces

ACTIVE
- tightening memory policy into the live provider lane

NEXT
- validate
- commit
- report exact milestone
```

### 2. Milestone Update

Use after a completed slice.

```text
⌜rmnzr⌟ ⌜✦ JIMI SUPERSTAR⌟  :D

RETROBUILDER  [████████░░] 82%  R4 live loops materializing
L1GHT         [████████░░] 82%  L4 live surfaces
SLICE         [█████████░] 90%  provider context packet

DONE
- live lane now receives sovereign context instead of raw intent
- validation green
- checkpoint committed

NEXT
- provider response compaction
```

### 3. Pause / Explanation Mode

Use when the user asks where the project stands.

```text
⌜rmnzr⌟ ⌜✦ JIMI SUPERSTAR⌟  :)

RETROBUILDER  [████████░░] 82%  vision proved, live materialization active
L1GHT         [████████░░] 82%  runtime + live surfaces working together

STATE
- the house already thinks, remembers, and executes
- current effort is refining orchestration quality, not bootstrapping from zero

FOCUS
- memory compaction
- multi-provider adapter layer
- marketplace-capable capsule runtime
```

## Required Final Closeout Footer

At the end of meaningful implementation updates, include a compact footer:

```text
STATUS NOW
- RETROBUILDER: R4
- L1GHT: L4
- validation: green
- next frontier: provider response compaction
```

## Slice Bars

In addition to global bars, report one local bar for the active subsystem when helpful.

Recommended subsystem set:

- `MEMORY`
- `RUNTIME`
- `COCKPIT`
- `PROVIDERS`
- `CAPSULES`
- `MARKETPLACE`
- `SECURITY`

Example:

```text
MEMORY        [███████░░░] 70%
PROVIDERS     [██████░░░░] 60%
COCKPIT       [████████░░] 80%
```

## Human-Friendliness Rules

- say what changed in plain language
- keep one emotional cue visible
- avoid sounding like CI logs
- never hide real uncertainty
- when blocked, show the block clearly instead of sounding cheerful by force

## Relationship To L1GHT

`L1GHT` supplies grounded structure.

`RETROBUILDER Chat UIX` supplies grounded narration.

One defines truth compression.
The other defines truth presentation.

## Relationship To JIMI

This UIX is the default conversation mode for JIMI SUPERSTAR while RETROBUILDER is the active work method.

Other work methods may introduce other communication rituals later, but this one is canonical for the current house.
