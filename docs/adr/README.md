# Architecture decisions

These records describe the authorized P0/P1 implementation choices. Foundational decisions are adopted for this implementation; provider and persistence work is explicitly provisional. They do not certify full Go parity.

- [ADR-001: Native Rust workspace](001-native-workspace.md)
- [ADR-002: Tokio runtime boundary](002-tokio-runtime.md)
- [ADR-003: Object-safe async contracts](003-dynamic-async.md)
- [ADR-004: Typed content with raw preservation](004-content-codec.md)
- [ADR-005: Stable state schema identities](005-state-schema.md)
- [ADR-006: One tool execution owner](006-tool-ownership.md)
- [ADR-007: Native persistence envelope](007-persistence-compatibility.md)
- [ADR-008: Private provider transport models](008-provider-transports.md)
- [ADR-009: Explicit capabilities and schema validation](009-capability-checks.md)
- [ADR-010: Owned streams and cancellation scopes](010-cancellation.md)
