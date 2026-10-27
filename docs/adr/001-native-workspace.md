# ADR-001: Native Rust workspace

Status: Adopted for the foundation. Date: 27 October 2026.

## Context

The root Rust implementation follows RUST_MIGRATION_PLAN.md and preserves the supplied Go source snapshot. This decision settles a boundary needed to implement or validate P0/P1.

## Decision

Use four library crates plus xtask, with the Go code only as a development reference.

## Alternatives and trade-offs

A line-by-line translation or Go FFI bridge would retain language/runtime constraints.

## Consequences

Core has no Tokio dependency; the facade can disable the runtime. Publication stays disabled until names and packaging are reviewed.

## Validation and follow-up

Evidence: `cargo check -p agent-framework --no-default-features --locked`. See the migration status for commands actually executed. Revisit when the first consuming provider/runtime use case exposes a concrete limitation.
