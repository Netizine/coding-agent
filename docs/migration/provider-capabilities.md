# Provider capability status

| Provider | Availability | Streaming | Tools | Structured output | Service history/background |
| --- | --- | --- | --- | --- | --- |
| Scripted test provider | Implemented, offline | Scripted updates | Descriptor validation only | Schema validation only | Disabled by default; configurable fixture behavior |
| OpenAI Responses/Chat | P3 planned | Not implemented | Not implemented | Not implemented | Not implemented |
| Azure OpenAI/Foundry | P8 planned | Not implemented | Not implemented | Not implemented | Not implemented |
| Anthropic/Gemini | P8 planned | Not implemented | Not implemented | Not implemented | Not implemented |
| Copilot | P9 planned | Not implemented | Not implemented | Not implemented | Not implemented |

ProviderCapabilities is a conservative draft. Validation rejects unsupported requested capabilities, including media, hosted tools, parallel calls, reasoning and opaque provider extensions. Tool execution ownership distinguishes framework and provider execution, but neither execution loop is implemented here. Protocol-specific capabilities remain with their adapters.

The [feasibility review](provider-feasibility.md) selects a private Foundry REST boundary and a pinned native Copilot SDK probe. Full endpoint coverage and live integration remain adapter-phase risks. MCP remains a future adapter, not a foundation dependency.
