# Compatibility

## Temporal Server

The v1 support floor is Temporal Server 1.29. Negotiation is authoritative:
the TUI reads `GetSystemInfo`, `DescribeNamespace`, and non-mutating endpoint
probes instead of inferring support from a version string. Press `K` to inspect
evidence for the active namespace.

The checked matrix uses checksum-verified official Temporal CLI releases and a
fresh random-port server per row:

| Server | Visibility / Update / Schedules / Deployments / Batch / Search Attributes | Worker heartbeats | Workflow pause |
| --- | --- | --- | --- |
| 1.29.1 | Available | Unavailable | Unavailable |
| 1.30.2 | Available | Available | Unavailable by default |
| 1.31.2 | Available | Available | Unavailable by default |

The full mutation contract separately runs 1.31.2 with
`frontend.WorkflowPauseEnabled=true` and verifies Update, pause/unpause, reset,
Schedules, Worker Deployments, Search Attributes, Codec round trips, and Batch
Operations with real Rust Workers.

`UNKNOWN` stays optimistic because a transient probe must not disable a usable
dashboard. `UNAVAILABLE` disables only its surface. `RESTRICTED` reports an
authorization failure without degrading unrelated APIs. Temporal Cloud is
governed by live evidence rather than a fixed Server row.

## Client and platforms

- Rust 1.97.1, edition 2024.
- Protobuf compiler 35.1 for source and release builds.
- Temporal Rust client 0.5.0 (Public Preview).
- Terminal minimum 58×16; release PTY coverage includes 80×24.
- Artifacts: Linux x86_64, macOS ARM64, macOS x86_64, Windows x86_64 MSVC.

Each platform runs unit/state/UI tests and an optimized binary smoke test.

```sh
scripts/install-temporal-cli.sh
cargo test --locked --test live_temporal -- --ignored --nocapture
scripts/compatibility.sh
```
