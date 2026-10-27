# ADR-003: Object-safe async contracts

Status: Adopted for the foundation. Date: 27 October 2026.

## Context

The root Rust implementation follows RUST_MIGRATION_PLAN.md and preserves the supplied Go source snapshot. This decision settles a boundary needed to implement or validate P0/P1.

## Decision

Providers, tools and provider-call middleware return boxed futures; provider streams own their request/client state.

## Alternatives and trade-offs

A fully generic Agent makes heterogeneous providers harder to compose; native async trait methods are not directly dyn-compatible.

## Consequences

Arc<dyn ModelProvider> and Arc<dyn FunctionTool> compile and execute in offline tests. Benchmark allocation overhead before optimizing.

## Validation and follow-up

Evidence: `crates/agent-test-support/tests/lifecycle.rs`. See the migration status for commands actually executed. Revisit when the first consuming provider/runtime use case exposes a concrete limitation.
