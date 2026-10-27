# Provider feasibility decisions

Reviewed 27 October 2026. These are P0 boundary decisions, not working provider adapters.

## Foundry: private REST transport with explicit endpoint coverage

The inspected [official Azure Rust SDK tree](https://github.com/Azure/azure-sdk-for-rust/tree/main/sdk) contains core/identity and several service SDKs but did not establish coverage of the exact AI Projects operations used by this snapshot. We will use a private HTTP adapter and Azure credentials behind the framework interfaces rather than require an unverified generated Foundry SDK. This is a scoped inference from the inspected source, not a claim that no third-party Rust client exists.

The frozen Go source defines these distinct surfaces:

| Surface | Baseline requirement | Rust implementation phase |
| --- | --- | --- |
| Project model deployment | `<project>/openai/v1/`, no `api-version` query, deployment name as model | P8 using P3 Responses transport |
| Server agent | `<project>/agents/<escaped-name>/endpoint/protocols/openai/`, `api-version=v1`; service owns instructions | P8 private route and request mapping |
| Shared authentication | `https://ai.azure.com/.default`, token refresh, origin guard | P8 credential abstraction and mock refresh tests |
| Hosted session/user identity | Sticky response/request session ID, conflict rejection, identity/header propagation, served-model metadata | P8 session middleware |
| Project administration | Project-root routing distinct from `/openai/v1/` | P8 explicitly enumerated operations |
| Memory stores | Get/create, scoped delete/search, begin asynchronous update; create-race conflict handling | P8 REST DTOs and polling policy |

Evidence: `provider/foundryprovider/{agent,client,hostedsession,memory}.go` in the source manifest. No full generated-client translation is needed for the core architecture. Remaining adapter risks are API-version drift, token-origin handling and long-running memory operations. They need fake HTTP fixtures and opt-in live verification in P8, not a claim of P0 service coverage.

## Copilot: native Rust SDK behind a private adapter

The [official Copilot SDK](https://github.com/github/copilot-sdk/tree/main/rust) supplies Rust APIs. The feasibility probe pins `github-copilot-sdk = 1.0.27` with default features disabled, using `Client::from_streams` and in-memory Tokio duplex streams. This avoids CLI downloads/startup, authentication and model calls. SDK types do not enter the framework's public API or normal Cargo dependency graph.

```text
cargo test --manifest-path tools/provider-spikes/Cargo.toml --target-dir target/provider-spikes --locked --offline
```

The probe checks JSON-RPC ping framing, response decoding and explicit shutdown/transport closure. It validates the native connection boundary only. Session creation/resume, event mapping, tool ownership, permissions and real CLI lifecycle remain P9. The SDK documents that abandoning `stop()` may require `force_stop()`; the adapter must use a bounded stop and force-stop fallback. A successful ping is not a live-model or protocol-conformance certification.

Dependency downloads are preparation steps. The separate lockfile records this research dependency; normal workspace tests and CI do not require the Copilot SDK.
