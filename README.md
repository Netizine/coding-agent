# Agent Framework for Rust

An independent Rust rewrite using the local Microsoft Agent Framework for Go snapshot as its behavioral reference. This repository currently implements the **P0/P1 foundation**, not a complete agent SDK.

The workspace has four library crates and a development task runner:

| Crate | Implemented responsibility |
| --- | --- |
| `agent-core` | Runtime-independent messages/content, annotations, response accumulation, filters, schemas, tool descriptors and capability declarations |
| `agent-runtime` | Object-safe async provider/tool contracts, provider middleware, cancellation/deadlines, owned task shutdown, stream collection and registered state-codec spikes |
| `agent-test-support` | Scripted provider, request recording and fixture loading |
| `agent-framework` | Public facade; disable default features for data contracts without Tokio |
| `xtask` | Reference integrity and migration evidence validation |

## Try it

Install Rust through rustup. The workspace selects Rust 1.99.0, edition 2024, with rustfmt and clippy. This is the tested initial minimum; older compiler support has not been established.

```text
cargo run -p agent-framework --example hello_scripted --locked
```

Expected output: `Hello from Rust.` No credentials or model service are required. Cargo downloads dependencies on the first build. The example uses a scripted model and does not run an agent tool loop.

## Verify

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p agent-framework --no-default-features --locked
cargo run -p xtask --locked -- check
cargo run -p xtask --locked -- baseline
```

The last command requires the preserved local Go directory. Other Rust tests and checks do not need Go or the reference source. After dependencies are fetched, add `--offline` to Cargo commands to disable Cargo's network access. This is distinct from an OS-level network sandbox.

CI is configured for Windows, Linux and macOS. Local validation results are recorded in [the implementation status](docs/migration/status.md); configuring CI is not evidence that remote jobs have run.

## Migration records

- [Overall roadmap](RUST_MIGRATION_PLAN.md)
- [Current implementation status and remaining gates](docs/migration/status.md)
- [Compatibility decisions](docs/migration/compatibility.md)
- [Baseline and inventory workflow](docs/migration/BASELINE.md)
- [Behavior ledger](docs/migration/parity.json)
- [Architecture decisions](docs/adr/README.md)
- [Provider capabilities](docs/migration/provider-capabilities.md)
- [Provider feasibility decisions](docs/migration/provider-feasibility.md)
- [P1 public API review](docs/migration/public-api-review.md)

The next implementation phase is the Agent lifecycle, sessions/history and tool loop. Cloud adapters, durable workflows, memory and protocol servers are not implemented. All packages are currently `publish = false`; the names are provisional.
