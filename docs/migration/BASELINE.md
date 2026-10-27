# Reference provenance and acquisition

The supplied directory has no Git metadata. Its declared module is `github.com/microsoft/agent-framework-go`; that declaration does not prove an upstream commit. The baseline remains an **unverified-origin local snapshot** with a verified content identity.

The source fingerprint is `e89a0c375daec01bbf0bdd3d919e5138b9ebd7ad51c4ac9aad48ca12153f0b11`, covering 673 files, including 604 Go files and 180 Go test files. The exact algorithm and individual hashes are in `source-baseline.json`.

## Preserve and recover

Keep the source folder unchanged. An exact local recovery archive was created and restored into a separate directory, with all 673 file hashes verified. Its upstream revision remains unknown; do not fetch `main` as an automatic replacement. The archive is `reference/agent-framework-go.zip`, SHA-256 `463aba1c3251e6f7b65ad2fbb1ae5341e857e69407fb3bebff174d4d20ccf9b2`. Archive compression bytes are secondary to the manifest's per-file identity.

```text
go run tools/reference/main.go create reference/agent-framework-go.zip docs/migration/source-baseline.json temp/agent-framework-go
go run tools/reference/main.go verify reference/agent-framework-go.zip docs/migration/source-baseline.json
go run tools/reference/main.go restore reference/agent-framework-go.zip docs/migration/source-baseline.json target/reference-recovery
go test tools/reference/main.go tools/reference/main_test.go
```

Create requires a new archive path; restore requires a nonexistent destination. Verification rejects missing/extra/duplicate/corrupt entries and unsafe manifest paths. Recovery tests cover exact bytes, corrupt input, changed sources and overwrite refusal. On another machine, separately transfer the local archive, restore to `temp/agent-framework-go`, and run `cargo run -p xtask --locked -- baseline`.

The root Git repository ignores `temp/` and `reference/*.zip`, as requested; it tracks the fingerprint and inventory. Therefore cloning the Rust repository alone does **not** recover the Go reference. Normal Rust tests use checked-in fixtures and do not require it. No Go snapshot bytes are published via an archive workaround.

## Static inventory

```text
go run ./tools/go-inventory/main.go ./temp/agent-framework-go ./docs/migration/go-inventory.json
```

This parses all Go source files, including nested tooling and platform/build variants, without importing packages or executing initializers. It records declared exports, explicit fields/methods, test-name candidates and examples. It does not resolve embedded/promoted symbols, prove buildability, expand table-driven test cases, or certify parity. Test names are syntactic candidates beginning with `Test`, not runtime test enumeration.

## Baseline test selection

The selected tests were inspected for network/process entrypoints before execution. They use in-memory data, fake transports and temporary-file operations; they do not instantiate live model services. Run with `GOTOOLCHAIN=local`, `GOWORK=off` and `GOFLAGS=-mod=readonly` from the reference directory:

```text
go test ./message ./message/messagefilter ./internal/jsonx
go test ./agent ./agent/format/jsonformat ./tool/functool ./message/... ./embedding/... ./vectordata/... ./workflow/...
go test ./provider/openaiprovider -run '^TestStrict'
```

All commands above passed on Windows. Go build/module caches change outside the reference; missing dependencies were fetched explicitly during preparation/execution. These are credential-free test runs, not OS-network-isolated executions. Verify the full source fingerprint after reference-side commands. Remaining provider/integration suites require separate audit in their phases; this selection is not a whole-repository certification.

## Differential response oracle

```text
go -C tools/go-oracle mod download
go -C tools/go-oracle run -mod=readonly . ../../tests/conformance/response-cases.json
go -C tools/go-oracle run -mod=readonly . ../../tests/conformance/content-cases.json content
```

The separate module references the source through a local `replace`; it does not edit the reference's go.mod/go.sum. The oracle and Rust tests consume the same eight expected response projections and thirteen content/coalescing cases. These fixtures cover selected semantics, not every provider behavior.

Source updates require explicit review: preserve the old manifest, inventory the new copy, assess changed behaviors, regenerate affected fixtures, and update ledger evidence. Never silently overwrite the baseline while validating it.
