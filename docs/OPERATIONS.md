# Operations guide

## Preflight

1. Start with `temporal tui --profile production --read-only`.
2. Confirm address, namespace, and `READ ONLY` in the header.
3. Press `K`; inspect `RESTRICTED`, `UNAVAILABLE`, or `UNKNOWN` evidence.
4. Verify Visibility queries and cursor pages before control mode.

Use TLS for non-loopback frontends. Store API keys with
`profile set-api-key`; use environment references in headless sessions.
Credentials in addresses/public headers are rejected.

For a protected self-hosted deployment, establish and verify the local session
before the read-only preflight:

```sh
temporal tui --profile production auth login \
  --url https://temporal.example.com \
  --username operator
temporal tui --profile production auth whoami
temporal tui --profile production --read-only
```

The login client is built in and the launched TUI connects directly to Temporal
gRPC. Temporal CLI only dispatches the extension and waits for it; it never
proxies dashboard traffic. Standalone `temporal-tui` accepts the same arguments
and has no Temporal CLI runtime dependency.

## Observation

- `1`: Workflows, details, history, payloads, chains, pending Activities.
- `2`: Task Queue backlog, rates, pollers, limits, routing.
- `3`: Worker heartbeat resources, slots, outcomes, cache, SDK metadata.
- `4`: Deployment Current/Ramping routing and drainage.
- `5`: Schedule definitions, policies, recent/future actions.
- `6`: server-side Batch jobs.
- `A`: Search Attribute registry; `P`: verified atomic profile switch.

Use `r` for manual incident refresh; `a` toggles automatic refresh.

## Mutation procedure

1. Narrow and inspect the target.
2. Confirm namespace and `CONTROL`.
3. Read the frozen identity/query preview.
4. Type the exact requested ID for destructive/broad actions.
5. Wait for the terminal result; submission is not completion.
6. Refresh and verify server state.

Batch execution remains one Temporal server-side job over a non-empty frozen
query. Worker Deployment changes retain Temporal's missing-queue/no-poller
protections; verify routing propagation and drainage afterwards.

## Codec operation

A Codec Server sees plaintext by design. Use a trusted TLS endpoint, secret
header references, and scoped authorization. Codec failures affect payload
presentation/encoding; use the explicit operation result to determine Temporal
success or failure.

## Local-session operation

- Prefer the masked password prompt. For automation, pipe the password to
  `auth login --password-stdin`; never place it in process arguments.
- Keep the OS credential manager unlocked while starting or switching to the
  profile. Refresh credentials are never copied into profile TOML.
- Concurrent TUI and `auth whoami` processes serialize refresh for the same
  profile/origin/user and reload the latest one-time credential before use.
- Use `auth whoami` to check the current identity and session status.
- Use `auth logout` before account handoff or workstation decommissioning. It
  revokes the refresh credential before removing the local copy.
- If a process or network connection dies during refresh, the server may
  already have consumed the one-time credential. Sign in again if retrying
  reports an invalid or expired session.
- Disabling or resetting the server-side session stops future refresh. An
  access token already held by a running TUI remains usable until its short
  expiry, so also enforce server-side authorization when immediate cutoff is
  required.

## Recovery

- `ctrl-c` globally exits and restores raw mode, cursor, and alternate screen.
  After an external hard kill on Unix, use `stty sane`.
- Failed profile switching retains the old service and state.
- Failed migration retains the schema-1 or schema-2 source and reports the
  blocker.
- Exports are private create-new JSON files. Review redaction before sharing.

Every `master` merge runs locked quality, platform, and disposable 1.31 live
gates. Compatibility and dependency policy also run weekly.
