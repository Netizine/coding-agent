# ADR-005: Stable state schema identities

Status: Registry and fresh-process spikes validated. Date: 27 October 2026.

## Context

The root Rust implementation follows RUST_MIGRATION_PLAN.md and preserves the supplied Go source snapshot. This decision settles a boundary needed to implement or validate P0/P1.

## Decision

Encode schema ID, schema version and envelope version, then explicitly select the decoder.

## Alternatives and trade-offs

Rust TypeId or type_name strings are not stable persistence identifiers.

## Consequences

The envelope and explicit codec registry restore custom values and queued/pending-request fixtures in fresh processes. Wrong IDs, versions, types, duplicate registration and corrupt payloads fail. This is not a checkpoint manager; graph identity, schema migrations and execution/replay are P5.

## Validation and follow-up

Evidence: `crates/agent-test-support/tests/lifecycle.rs`. See the migration status for commands actually executed. Revisit when the first consuming provider/runtime use case exposes a concrete limitation.
