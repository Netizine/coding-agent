# ADR-009: Explicit capabilities and schema validation

Status: Draft implemented. Date: 27 October 2026.

## Context

The root Rust implementation follows RUST_MIGRATION_PLAN.md and preserves the supplied Go source snapshot. This decision settles a boundary needed to implement or validate P0/P1.

## Decision

Reject requests for unsupported capabilities. Generate schemas with schemars and validate separately with jsonschema.

## Alternatives and trade-offs

Silently ignored options make provider interchangeability misleading.

## Consequences

Remote/file schema retrieval features are disabled. Current capabilities are a draft; media, reasoning, parallel tools and hosted tools must be added before provider parity.

## Validation and follow-up

Evidence: `crates/agent-core/tests/contracts.rs`. See the migration status for commands actually executed. Revisit when the first consuming provider/runtime use case exposes a concrete limitation.
