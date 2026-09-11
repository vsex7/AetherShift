# AetherShift Project Governance & Maintenance Policy

## 1. Versioning Policy (SemVer 2.0.0)
AetherShift strictly adheres to Semantic Versioning (`MAJOR.MINOR.PATCH`):
- **MAJOR**: Breaking changes to IPC protocol, core traits, or configuration backward compatibility.
- **MINOR**: New compositor backends, layout modes, plugin capabilities, or CLI commands.
- **PATCH**: Bug fixes, security patches, internal performance optimizations.

## 2. Backward Compatibility Guarantees
- **Zero-Mutation Core**: The core daemon will never write to system configurations (`/etc`, `~/.config/hypr/`) without explicit user opt-in.
- **Protocol Stability**: Existing IPC Request/Response wire shapes are append-only. Deprecated fields remain valid across at least one minor release cycle.
- **Clean Fallback**: Failure to connect to a compositor backend or plugin immediately triggers fallback to safe baseline mode.

## 3. Contributing Guidelines
- **Compositor Backends**: Implement the `CompositorBackend` trait in `aethershift-core/src/backend.rs`.
- **Custom Actions**: Provide a standalone executable and a `plugin.toml` adhering to the Plugin Specification.
- **Presets**: Submit TOML presets to `presets/` with conflict validation.
