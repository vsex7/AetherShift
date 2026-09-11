# AetherShift Security & Vulnerability Disclosure

## 1. Security Architecture
- **Unix Domain Socket Permissions**: The daemon socket is bound exclusively to the current user runtime directory (`/run/user/$UID/aethershift.sock`) with mode `0600`.
- **Plugin Sandbox & Scope**:
  - Plugins execute with non-privileged process credentials.
  - Strict timeouts (default 1000ms) prevent denial-of-service hanging.
  - Payloads exceeding 64KB are rejected.
- **Zero-Keylogging Invariant**: AetherShift only monitors registered window-manager dispatch shortcuts; standard keyboard inputs are never intercepted, inspected, or logged.

## 2. Reporting Security Issues
To report a security vulnerability or privilege escalation bug, please contact:
- Security contact: `security@aethershift.org` (or open a confidential GitHub Advisory).
- Do not disclose issues publicly until a patch has been released.
