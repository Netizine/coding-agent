# P1 public API review

Reviewed 27 October 2026 against the original P1 exit criteria. The workspace remains unpublished (`publish = false`).

| Contract | Review outcome and evidence |
| --- | --- |
| Core dependency boundary | Core has no Tokio dependency. Facade compiles with `--no-default-features`; network provider SDKs are absent from its dependency graph. |
| Provider/tool polymorphism | Boxed futures and owned `Send` streams work through trait objects; scripted provider and dynamic typed tool tests exercise calls. |
| Content extensibility | Known variants reject malformed shapes; unknown discriminators and extensions survive JSON round trips. Shared fixtures cover all newly added native families. |
| IDs and defaults | Domain-specific IDs validate construction/serde; wire message IDs retain empty-string behavior. Run option patches retain false/zero/empty and clear optional values explicitly. |
| Schema contracts | Generation, validation and strict transforms operate on owned values; external references cannot fetch network/file resources. No live provider acceptance is inferred. |
| Lifecycle ownership | Child scopes prevent sibling cancellation. Owned tasks support awaited bounded shutdown and abort on drop. Stateful provider shutdown must be integrated by each adapter. |
| Durable value boundary | Explicit codec ID/version registration works across processes for values, queue payloads and pending requests. No compiler type name is serialized. |
| Example ergonomics | Public facade example and doctest compile/run without credentials. Constructor/default patterns avoid callers filling every optional field. |

Intentional API differences are recorded in compatibility.md: deep owned clones, raw-object shared identity, nil/empty map normalization, stricter decoding, overflow saturation and schema depth limits. These have bounded tests or explicit documented policies, rather than a promise of byte-identical Go objects.

Future API expansion belongs to its implementation phase: Agent/session/history/tool loops in P2, concrete HTTP DTOs in P3/P8, graph/checkpoint replay in P4/P5 and protocol adapters in P9. Package inventory rows retain their later whole-package review obligations; P1 completion certifies the foundation contracts and fixtures only.
