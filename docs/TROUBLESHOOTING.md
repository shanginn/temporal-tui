# Troubleshooting

## Connection and authentication

| Symptom | Check |
| --- | --- |
| Invalid address | Use `host:7233` or HTTP(S). Paths, query, fragments, credentials, and other schemes are rejected. |
| TLS/issuer error | Check `--tls`, CA, server name, and complete client cert/key pair. API keys enable TLS. |
| `PermissionDenied` in `K` | The credential cannot call that read endpoint. Fix policy; unrelated views remain usable. |
| Secret variable missing | Export the profile's exact variable or use `profile set-api-key`. |
| Credential manager error | Unlock the OS store; Linux headless sessions may need environment references. |

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

The file needs an integer `schema_version`. v1 creates
`config.toml.v1.bak`. If it already differs, move both files to a recovery
directory and choose the authoritative copy; the client never overwrites it.
A newer schema requires a newer binary.

## Terminal/display

- Minimum is 58×16.
- Use `--no-color`, `[ui] color = false`, or `NO_COLOR`.
- `ctrl-c` exits everywhere; after a hard kill on Unix run `stty sane`.
- In headless sessions without clipboard, use private JSON export.

For a report, include binary/Server versions, namespace, capability evidence,
exact error, and reproduction. Never attach credentials or unreviewed exports.
