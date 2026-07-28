# Troubleshooting

## Connection and authentication

| Symptom | Check |
| --- | --- |
| Invalid address | Use `host:7233` or HTTP(S). Paths, query, fragments, credentials, and other schemes are rejected. |
| TLS/issuer error | Check `--tls`, CA, server name, and complete client cert/key pair. API keys enable TLS. |
| `PermissionDenied` in `K` | The credential cannot call that read endpoint. Fix policy; unrelated views remain usable. |
| Secret variable missing | Export the profile's exact variable or use `profile set-api-key`. |
| Credential manager locked/unavailable | Unlock the OS store and retry. Local login cannot safely fall back to profile TOML; Linux headless sessions need a working Secret Service/keyring. |
| Local session expired or revoked | Run `temporal-tui --profile NAME auth whoami`; if renewal is no longer valid, run `auth logout` and then `auth login` again. |
| Login URL or redirect rejected | Use the HTTPS origin of the auth service. Advertised token endpoints must stay on that origin; redirects are never followed. |
| `--allow-http` rejected | The hidden development switch accepts HTTP only for a loopback auth endpoint. Plaintext Temporal transport is also loopback-only; use TLS everywhere else. |

A reset, disabled account, or revoked refresh credential prevents future
renewal. A running TUI may retain its already-issued short-lived access until
that token expires. `auth logout` revokes the refresh credential before
removing its local copy; if revocation fails, fix connectivity and retry.

A crash or network loss can occur after the server consumes a one-time refresh
but before the replacement reaches the credential manager. If retrying still
reports an invalid or expired session, sign in again; the exchange cannot be
made atomic entirely from the client.

## Empty or incomplete views

- Confirm namespace and clear/correct the Visibility or Schedule query.
- `[`/`]` page cursor-backed lists; counts are separate server calls.
- Enter undiscovered Task Queue names with `/`.
- Worker heartbeat data requires Server 1.30+ and heartbeat-emitting Workers.
- Workflow pause is feature-gated and disabled by default in the matrix.

## Mutation disabled

- `READ ONLY` blocks all writes; global `--read-only` survives profile switches.
- Closed Workflows cannot receive running-only actions.
- Unsupported/restricted capabilities disable only their actions.
- Wait for an in-flight Workflow mutation.
- Confirmations are case-sensitive and bind to the frozen target.

## Codec failures

Verify `{namespace}`, `/encode` and `/decode`, TLS, and secret auth. Embedded
credentials, redirects, oversized/invalid responses, and header controls are
rejected. Only one transient transport retry occurs.

## Config migration

The file needs an integer `schema_version`. Schema 1 and 2 create
`config.toml.v1.bak` and `config.toml.v2.bak`, respectively, before migration
to schema 3. If the matching backup already differs, move both files to a
recovery directory and choose the authoritative copy; the client never
overwrites it. A newer schema requires a newer binary.

## Terminal/display

- Minimum is 58×16.
- Use `--no-color`, `[ui] color = false`, or `NO_COLOR`.
- `ctrl-c` exits everywhere; after a hard kill on Unix run `stty sane`.
- In headless sessions without clipboard, use private JSON export.

For a report, include binary/Server versions, namespace, capability evidence,
exact error, and reproduction. Never attach credentials or unreviewed exports.
