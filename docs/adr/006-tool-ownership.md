# ADR-006: One tool execution owner

Status: Contract adopted; execution pending. Date: 27 October 2026.

## Context

The root Rust implementation follows RUST_MIGRATION_PLAN.md and preserves the supplied Go source snapshot. This decision settles a boundary needed to implement or validate P0/P1.

## Decision

Capabilities explicitly select framework-owned or provider-owned tool execution.

## Alternatives and trade-offs

Adding a harness tool loop around provider-owned tools can execute a call twice.

## Consequences

The distinction is represented but no tool loop or approval binding is implemented in P1. Invocation-count and replay tests are P2.

## Validation and follow-up

Evidence: `crates/agent-core/src/tool.rs`. See the migration status for commands actually executed. Revisit when the first consuming provider/runtime use case exposes a concrete limitation.
