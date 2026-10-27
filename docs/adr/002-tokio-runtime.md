# ADR-002: Tokio runtime boundary

Status: Adopted for the foundation. Date: 27 October 2026.

## Context

The root Rust implementation follows RUST_MIGRATION_PLAN.md and preserves the supplied Go source snapshot. This decision settles a boundary needed to implement or validate P0/P1.

## Decision

Keep messages, schema validation and response folding synchronous; place I/O lifecycle contracts in agent-runtime using Tokio.

## Alternatives and trade-offs

A runtime-generic executor adds complexity before any provider exists.

## Consequences

Callers supply a runtime; the library never creates one. Rust 1.99.0 and edition 2024 are the initial tested baseline.

## Validation and follow-up

Evidence: `crates/agent-test-support/tests/lifecycle.rs`. See the migration status for commands actually executed. Revisit when the first consuming provider/runtime use case exposes a concrete limitation.
