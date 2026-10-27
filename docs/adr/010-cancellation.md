# ADR-010: Owned streams and cancellation scopes

Status: Pull-based spike validated. Date: 27 October 2026.

## Context

The root Rust implementation follows RUST_MIGRATION_PLAN.md and preserves the supplied Go source snapshot. This decision settles a boundary needed to implement or validate P0/P1.

## Decision

Every invocation gets a child token. Initialization drop, stream drop, terminal errors and completion cancel that scope; deadlines wake pending consumers.

## Alternatives and trade-offs

Detached background producers and parent-token cancellation can leak work or cancel unrelated runs.

## Consequences

The provider invocation path spawns no producer task. `RunTasks` supplies owned spawning and explicit bounded asynchronous shutdown. Tests cover cooperative cleanup, timeout abort-and-join, parent isolation and abandoned shutdown. Concrete provider clients must integrate ownership and awaited shutdown in P2/P3; Drop can abort but cannot synchronously await task destruction.

## Validation and follow-up

Evidence: `crates/agent-test-support/tests/lifecycle.rs`. See the migration status for commands actually executed. Revisit when the first consuming provider/runtime use case exposes a concrete limitation.
