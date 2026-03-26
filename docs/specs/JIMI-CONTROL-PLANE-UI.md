# JIMI Control Plane UI

This document defines the integrated control plane surface for ANOMALY.

The cockpit should not only observe the house. It should also configure, steer, inspect, and eventually govern it.

## Purpose

The control plane turns the visual interface into four things at once:

- cockpit
- configurator
- memory console
- policy center

## Design Principle

Every panel should answer one of these questions:

1. what is the house doing
2. how is the house configured
3. what does the house remember
4. which powers are enabled
5. where does the project stand

## Canonical Surfaces

### 1. House Control

Controls the active operating posture of the house.

Primary contents:

- active work method
- active project phase
- runtime mode
- operator-facing communication mode
- active frontier

### 2. Mandala Studio

Controls identity and personality state.

Primary contents:

- installed mandalas
- active slot
- active snapshot
- stable memory
- execution policy
- memory policy

### 3. Capsule Manager

Controls installable bodies of the house.

Primary contents:

- installed capsules
- versions
- origins
- dependencies
- slot bindings
- activation readiness

### 4. Provider Matrix

Controls execution engines and routing.

Primary contents:

- connected providers
- model per lane
- routing mode
- fallback chain
- lane health
- provider affinity per mandala

### 5. Memory Console

Controls and inspects memory flow.

Primary contents:

- hot / warm / archive capsules
- query layer
- summaries
- bridges
- triggers
- promotions
- compression metrics

### 6. World State Console

Controls and inspects host-state and machine-state truth.

Primary contents:

- indexed regions
- active-region cache
- workspace snapshots
- process and app state
- excluded or sealed regions
- preview of `world_state_slice` entering a `ContextPacket`

### 7. Security Surface

Controls safety and authority boundaries.

Primary contents:

- security class
- approvals
- fieldvault sealing rules
- provider permissions
- tool permissions
- filesystem / runtime scope

### 8. Workflow Composer

Controls execution flows.

Primary contents:

- build modes
- review flows
- patch flows
- architecture flows
- future storm flows

### 9. Macro Planner

Controls project-wide understanding.

Primary contents:

- project panorama
- milestones
- done / doing / next / later
- blockers
- recommended next move

### 10. Marketplace Shell

Controls external installable ecosystem.

Primary contents:

- discoverable capsules
- personality slots
- price / license / trust
- compatible providers
- install and bind flow

## Navigation Model

The control plane should be navigable by:

- visible panels in the cockpit
- future tabs or command palette
- natural language requests in chat

The chat should remain the fastest entrypoint.
The cockpit should remain the clearest global surface.

## Build Order

Recommended implementation order:

1. Macro Planner
2. House Control
3. Mandala Studio
4. Provider Matrix
5. Memory Console
6. World State Console
7. Security Surface
8. Capsule Manager
9. Workflow Composer
10. Marketplace Shell

## Read-First Rule

Before making every surface mutable, the control plane should first expose:

- current state
- intended target state
- safe actions
- blocked actions

This avoids fake configurators that look editable but are not truthful.

## Relationship To RETROBUILDER

RETROBUILDER gives the house a method.

The control plane gives the operator a steering wheel for that method.

## Relationship To L1GHT

`L1GHT` structures truth.

The control plane reveals and edits that truth safely.
