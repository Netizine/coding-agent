# ADR-008: Private provider transport models

Status: Adopted transport boundary; service adapters remain P3/P8/P9. Date: 27 October 2026.

## Context

The root Rust implementation follows RUST_MIGRATION_PLAN.md and preserves the supplied Go source snapshot. This decision settles a boundary needed to implement or validate P0/P1.

## Decision

Prefer private HTTP/SDK models behind framework contracts and select adapters against the exact required endpoints.

## Alternatives and trade-offs

Exposing vendor SDK types couples all users to vendor changes; translating the entire generated Foundry client expands scope.

## Consequences

Foundry uses private REST models with separately inventoried project, server-agent and memory routes. Copilot has a native Rust SDK, isolated in a development-only pinned mock-transport probe. Neither is a production adapter. SDK discovery does not establish full endpoint coverage.

## Validation and follow-up

Evidence: [provider feasibility](../migration/provider-feasibility.md) and `tools/provider-spikes`. Adapter risks and remaining live/conformance checks are named there.
