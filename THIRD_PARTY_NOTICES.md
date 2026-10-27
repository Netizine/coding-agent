# Third-party notices

This is an independent Rust implementation based on the local Microsoft Agent Framework for Go source snapshot. It is not an official Microsoft SDK.

The reference code is copyright Microsoft Corporation and licensed under the MIT License. The root LICENSE preserves that notice and permission text. Rust response accumulation and conformance scenarios adapted from the reference are covered by this attribution.

The exact local reference files and hashes are recorded in `docs/migration/source-baseline.json`. No upstream commit has been verified. Do not substitute a current upstream checkout and claim it is the same baseline.

Cargo dependencies retain their own licenses. `Cargo.lock` records selected versions. Dependency notice review and distributable third-party notices remain a release gate; this file does not assert every transitive dependency is MIT-licensed.

The Go source, its logo/assets and upstream GitHub automation are not included in Rust packages. Original source notices remain untouched under `temp/agent-framework-go`.
