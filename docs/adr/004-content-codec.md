# ADR-004: Typed content with raw preservation

Status: Adopted for the foundation. Date: 27 October 2026.

## Context

The root Rust implementation follows RUST_MIGRATION_PLAN.md and preserves the supplied Go source snapshot. This decision settles a boundary needed to implement or validate P0/P1.

## Decision

Use typed standard content, explicit extension maps and discriminator-aware raw fallback. Retain large JSON integers.

## Alternatives and trade-offs

Discarding unknown variants/fields would break forward compatibility; JSON-only APIs lose typed ergonomics.

## Consequences

Malformed known variants fail. All 21 baseline content discriminators have typed payloads; future unknown variants remain raw JSON. Additional provider fields may use the extension map. Values are preserved, not original whitespace/key order. Runtime-only SDK objects are shared, redacted and excluded from serialization.

## Validation and follow-up

Evidence: `crates/agent-core/tests/contracts.rs`. See the migration status for commands actually executed. Revisit when the first consuming provider/runtime use case exposes a concrete limitation.
