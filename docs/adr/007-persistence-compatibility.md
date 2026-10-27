# ADR-007: Native persistence envelope

Status: Adopted planning constraint. Date: 27 October 2026.

## Context

The root Rust implementation follows RUST_MIGRATION_PLAN.md and preserves the supplied Go source snapshot. This decision settles a boundary needed to implement or validate P0/P1.

## Decision

Version Rust durable formats and treat a Go importer as separately scoped work.

## Alternatives and trade-offs

Reproducing all Go reflected type identities would constrain the native design.

## Consequences

No existing user data was identified. The value-envelope spike is explicit and does not imply Go checkpoint compatibility.

## Validation and follow-up

Evidence: `docs/migration/compatibility.md`. See the migration status for commands actually executed. Revisit when the first consuming provider/runtime use case exposes a concrete limitation.
