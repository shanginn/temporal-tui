# Threat model

This model covers the `temporal-tui` client and release artifacts. It does not
replace Temporal Server hardening, namespace authorization, network policy, or
workstation security.

## Assets and trust boundaries

Protected assets are Temporal credentials, decoded payloads, Workflow and
Schedule identities, mutation authority, diagnostic exports, configuration,
and terminal state. Data crosses the Temporal gRPC connection, an optional
local-login HTTPS connection, an optional Codec HTTP connection, the OS
credential manager/environment, clipboard and export boundaries, and the
GitHub release boundary.

Temporal fields, payloads, failures, Codec responses, and profile files are
untrusted. A local user with the same OS account is trusted to edit that
account's non-secret configuration and read its terminal.

## Primary threats and controls

| Threat | Controls |
| --- | --- |
| Credential disclosure | Local-login passwords are masked or read from stdin and have no CLI flag. Refresh credentials exist only in the OS credential manager; access tokens are memory-only. Config stores non-secret metadata/references, sensitive public headers are rejected, and diagnostics redact sensitive key names. |
| Token theft or replay | Access tokens are short-lived. Refresh credentials rotate, are origin/user-bound in the OS store, are coordinated across processes, and are persisted before a newly issued access token is exposed. Logout revokes before local deletion. |
| URL/header smuggling | Temporal, login, Web UI, and Codec URLs reject embedded credentials. Temporal endpoints reject paths, queries, fragments, control bytes, and unsupported schemes. Header values reject NUL, CR, and LF after secret resolution too. |
| Login-origin confusion | Local login requires HTTPS, disables redirects, and accepts only same-origin advertised token endpoints. The hidden insecure override permits HTTP only for loopback auth and plaintext Temporal endpoints. |
| Network interception | TLS, custom CA, server-name override, API-key TLS, and mTLS are supported. Credential-bearing non-loopback connections require TLS. |
| Malicious Codec Server | Only HTTP(S); no URL credentials/fragments or redirects; bounded time/size and one transient retry. A Codec Server necessarily sees plaintext it decodes. |
| Terminal escape/bidi injection | CLI identity responses reject unsafe controls, bidi marks, and oversized fields. Every final Ratatui cell is also stripped of C0/C1 controls, OSC/ESC bytes, and Unicode bidi override/isolate controls before flush. |
| Mutation redirected by refresh/switch | Commands freeze namespace and exact identities; request IDs reject stale responses; profile handoff verifies before atomic swap and invalidates old requests. |
| Accidental destructive/broad mutation | CLI/profile read-only blocks all writes. Destructive and broad actions preview targets and require exact typed confirmation. Batch targets remain one frozen server-side query. |
| Config corruption/permission regression | Migration creates a byte-identical backup before atomic replacement. Both are `0600` on Unix. Unknown/newer schemas and conflicting backups stop migration. |
| Export overwrite/path traversal | Constrained filename components, create-new writes, `0600` on Unix, and structured redaction. |
| Compromised release | SHA-256, CycloneDX SBOM, GitHub provenance, and immutable Action commit pins. |

## Residual risks

- The official Temporal Rust SDK is Public Preview. Its transitive `backoff`
  and `instant` crates are unmaintained and have no safe replacement in the
  current SDK graph. `deny.toml` tracks both explicitly; current RustSec
  scanning finds no known vulnerability in them.
- Secrets exist in memory while connected. OS compromise, a debugger, or an
  unlocked credential manager is outside this boundary.
- Environment variables and piped standard input may be visible to privileged
  local processes. Prefer the masked prompt and OS credential manager.
- Revoking, disabling, or resetting a refresh session stops future renewal but
  cannot retract an access token already issued to a running process. That token
  can remain usable until its short expiry or server-side authorization denies
  it.
- The auth server consumes a one-time refresh token before the replacement
  reaches the client. A process crash, network loss, or credential-manager
  failure in that interval can strand the session and require a fresh login;
  client-side persistence ordering cannot make that server exchange atomic.
- Clipboard contents are visible to other local apps. Redaction is key-name
  based and cannot identify every domain secret; review exports before sharing.
- A configured Codec Server sees plaintext. Opening Web UI hands identity data
  to the browser. Those systems are separate trust boundaries.
- Temporal authorization is authoritative. Client safety gates reduce operator
  error but are not an authorization boundary.

## Verification

```sh
cargo audit
cargo deny --all-features --locked check
scripts/check.sh
scripts/compatibility.sh
```

Live tests use disposable random loopback ports and never the user's `7233`.
The local-login acceptance test uses isolated auth and bearer-gated gRPC
fixtures; it does not perform a positive login against production.
