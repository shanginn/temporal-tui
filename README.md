# temporal-tui

`temporal-tui` is a keyboard-first Temporal dashboard and control plane built
with Rust and [Ratatui](https://ratatui.rs/). It connects directly to a Temporal
frontend through the official Rust client; the web UI and Temporal CLI are not
required at runtime.

## Capabilities

- Browse workflow visibility results with server-backed cursor pagination,
  approximate counts, and `GROUP BY` aggregations.
- Inspect full paginated history, failure causes and stacks, payloads, memo,
  Search Attributes, pending Activities, and every run in a Workflow chain.
- Discover and switch namespaces without reconnecting.
- Negotiate server and namespace capabilities from reported flags and
  non-mutating probes. Press `K` for evidence; unavailable or restricted APIs
  degrade only their own surfaces.
- Diagnose Workflow and Activity Task Queues with backlog size and age,
  add/dispatch rates, pollers, effective rate limits, and current/ramping
  Worker Deployment routing. Queue names are discovered from Workflows and
  heartbeat-enabled Workers, or can be entered directly.
- Inspect heartbeat-enabled Workers with host CPU/memory, poller counts, slot
  utilization, task outcomes, sticky-cache health, SDK version, and plugins.
- Inspect GA Worker Deployments, traffic ramping, routing propagation, and
  per-version drainage state without using the removed legacy Versioning APIs.
- Promote a tracked Worker Deployment build to Current, configure or clear a
  Ramping build and percentage, and retain Temporal's missing-queue/no-poller
  protections.
- Invoke Workflow Query and Update handlers with zero or more JSON arguments,
  decode their results, and display complete Update failures.
- Pause or unpause a running Workflow and reset it at an explicit history-event
  boundary. Pause availability follows the server-side
  `frontend.WorkflowPauseEnabled` capability.
- Browse and filter Schedules with cursor pagination; inspect their action,
  policies, recent/future runs, memo, Search Attributes, and decoded inputs.
- Create, update, pause, unpause, trigger, backfill, and delete Schedules.
  Schedule updates use the current conflict token and preserve fields the form
  does not change.
- Inspect the namespace Search Attribute registry and add or remove custom
  attributes with an explicit type and exact-name confirmation.
- Preview a frozen non-empty Visibility query, then start, inspect, paginate,
  or stop a Temporal server-side cancel, terminate, signal, or delete Batch
  Operation. Targets are never expanded into per-Workflow client calls.
- Refresh manually or automatically.
- Use named connection profiles and saved visibility queries. Switch profiles
  inside the TUI with `P`; the target connection is verified before the current
  service is replaced, and stale responses from the previous cluster are
  discarded.
- Store API keys in macOS Keychain, Windows Credential Manager, Linux Secret
  Service, or resolve them from an environment variable. Secret material is
  never written to the profile file.
- Run in enforced read-only mode.
- Copy Workflow identity, export a redacted JSON diagnostic bundle, or open the
  exact run in Temporal Web UI.
- Send a named signal with JSON input.
- Request graceful cancellation or terminate an exact workflow run.
- Connect to a local cluster, self-hosted Temporal, or Temporal Cloud with API
  key, TLS, mTLS, custom CA, server-name override, and repeated gRPC headers.
- Decode displayed payloads and encode outgoing signal, Query, Update, and
  Schedule payloads through a standard Temporal Codec Server. Namespace
  routing, secret auth headers, response-size limits, timeouts, redirect
  blocking, and one bounded transient-transport retry are enforced.
- Restore raw mode, the cursor, and the alternate screen on normal and error
  exits.
- Strip terminal control sequences and Unicode bidi overrides from every final
  rendered cell.

Cancellation and termination require typing the exact Workflow ID and are
unavailable in read-only mode.
Reset, Schedule trigger, backfill, deletion, Search Attribute changes, rollout
changes, and Batch Operations also require an exact target confirmation where
applicable. All mutations are unavailable in read-only mode.
Mutation commands retain both the workflow ID and run ID selected when the
confirmation opened, so a refresh cannot redirect an action to a different run.

The staged development and release plan is tracked in [ROADMAP.md](ROADMAP.md).
Production references:
[installation](docs/INSTALLATION.md),
[compatibility](docs/COMPATIBILITY.md),
[operations](docs/OPERATIONS.md),
[troubleshooting](docs/TROUBLESHOOTING.md),
[accessibility](docs/ACCESSIBILITY.md), and
[threat model](docs/THREAT_MODEL.md).

## Build

The repository pins Rust 1.97.1, including Rustfmt and Clippy:

```sh
rustup show
cargo build --release --locked
./target/release/temporal-tui --help
```

The release binary is `target/release/temporal-tui`.
Source builds also require the Protobuf compiler (`protoc`); release CI pins
the current stable 35.1 toolchain from checksum-verified official archives.
On macOS, the Xcode Command Line Tools must be installed and their license
accepted so Rust can invoke the system SDK and linker.

Install the latest release with Homebrew:

```sh
brew install --formula \
  https://github.com/shanginn/temporal-tui/releases/latest/download/temporal-tui.rb
```

Or build directly from GitHub:

```sh
cargo install --locked --git https://github.com/shanginn/temporal-tui
```

This version pins Ratatui 0.30.2, Crossterm 0.29.0,
`temporalio-client` 0.5.0, Tokio 1.53.1, and Temporal CLI 1.8.1. The official
Temporal Rust client is currently marked **Public Preview**, so review its
[support status](https://github.com/temporalio/sdk-rust) before adopting this
tool for critical production operations.

## Run

Against a local Temporal frontend:

```sh
./target/release/temporal-tui \
  --address 127.0.0.1:7233 \
  --namespace default
```

Against Temporal Cloud with an API key:

```sh
TEMPORAL_ADDRESS='your-namespace.account.tmprl.cloud:7233' \
TEMPORAL_NAMESPACE='your-namespace' \
TEMPORAL_API_KEY='your-api-key' \
./target/release/temporal-tui
```

With mTLS:

```sh
./target/release/temporal-tui \
  --address temporal.example.com:7233 \
  --namespace production \
  --tls \
  --tls-ca ./ca.pem \
  --tls-cert ./client.pem \
  --tls-key ./client.key
```

Run `temporal-tui --help` for visibility-query, refresh, page-size, TLS, and
custom-header options. Values read from API-key and private-key environment
variables are not echoed in the generated help.

### Connection profiles

Profiles live in the platform config directory (`temporal-tui config-path`
prints the exact file). Create and select one:

```sh
temporal-tui profile create cloud \
  --address your-namespace.account.tmprl.cloud:7233 \
  --namespace your-namespace \
  --tls \
  --web-ui-url https://cloud.temporal.io \
  --set-default
temporal-tui profile set-api-key cloud
temporal-tui --profile cloud
```

`profile set-api-key` reads without terminal echo and stores the value in the
operating-system credential manager. For headless environments, use
`profile create ... --api-key-env TEMPORAL_API_KEY`.

Press `P` from the dashboard to switch among configured profiles without
restarting. Profile rows contain only non-secret address, namespace, mode, and
Codec Server status. Secret references are resolved only after selection. A
failed reconnect leaves the current connection and dashboard state unchanged.

Save reusable Visibility queries without hand-editing TOML:

```sh
temporal-tui filter save failures \
  "ExecutionStatus = 'Failed' AND StartTime > '2026-07-27T00:00:00Z'"
```

Configure a Codec Server per profile. The endpoint can include
`{namespace}`; temporal-tui appends `/encode` or `/decode` and always sends
`X-Namespace`:

```sh
temporal-tui profile create encrypted-cloud \
  --address your-namespace.account.tmprl.cloud:7233 \
  --namespace your-namespace \
  --tls \
  --api-key-env TEMPORAL_API_KEY \
  --codec-endpoint 'https://codec.example/namespaces/{namespace}' \
  --codec-auth-env TEMPORAL_CODEC_AUTH
```

Codec authorization is resolved only at runtime. It is not written to the
profile or diagnostic exports.

Schema-2 config accepts persisted UI defaults:

```toml
[ui]
page_size = 200
refresh_seconds = 5
auto_refresh = true
color = true
```

CLI flags override numeric defaults and can disable auto-refresh or color.
`NO_COLOR` is honored. Schema-1 files migrate atomically after a byte-identical
`config.toml.v1.bak` is written.

## Keyboard controls

| Key | Action |
| --- | --- |
| `1` / `2` / `3` / `4` / `5` / `6` | Workflows / Task Queues / Workers / Deployments / Schedules / Batch Operations |
| `j` / `k`, arrows | Move through the active list or Workflow history |
| `g` / `G`, home / end | Jump to first or last item |
| `tab` / `enter` | Switch between Workflow and history panes |
| `/` | Edit Visibility query, or enter a Task Queue name |
| `f` | Select a saved visibility query |
| `#` | Show `GROUP BY` counts |
| `n` | Switch namespace |
| `P` | Switch configured connection profile |
| `A` | Inspect and manage namespace Search Attributes |
| `K` | Inspect negotiated server/namespace capabilities |
| `[` / `]` | Previous / next page in the active paginated view |
| `r` | Refresh now |
| `a` | Toggle automatic refresh |
| `H` | Load the next older history page |
| `C` | Show the Workflow chain |
| `v` | Inspect payloads, failures, memo, and Search Attributes |
| `y` | Copy Workflow ID and Run ID |
| `e` / `o` | Export redacted JSON / open Temporal Web UI |
| `s` | Send a signal with JSON input |
| `Q` / `U` | Invoke a Query / Update with a JSON argument array |
| `p` | Pause or unpause the selected Workflow or Schedule |
| `R` | Reset a Workflow at a history event (exact-ID confirmation) |
| `C` / `R` on Deployments | Set Current / configure Ramping build |
| `c` | Request workflow cancellation |
| `x` | Terminate a workflow |
| `N` / `E` | Create / edit a Schedule |
| `t` / `b` / `d` | Trigger / backfill / delete a Schedule |
| `N` / `s` on Batches | Preview and start / stop a server-side Batch Operation |
| `?` | Open keyboard help |
| `q` / `ctrl-c` | Quit |

## Tests

Run the complete static, unit, UI-rendering, and release-build gate:

```sh
scripts/check.sh
```

The live contract test starts an ephemeral Temporal dev server and verifies
cluster discovery, namespaces, visibility cursors and counts, complete history
pagination, Task Queue backlog, Worker and Worker Deployment endpoints, payload
redaction, an encode/decode Codec Server round trip, Workflow chains, signal,
cancel, terminate, Query, Update, Workflow pause/reset, and the complete
Schedule lifecycle, Search Attribute registration, Worker Deployment rollout,
and server-side Batch Operation lifecycle through the real gRPC adapter and a
real Rust SDK Worker:

```sh
scripts/install-temporal-cli.sh
cargo test --locked --test live_temporal -- --ignored --nocapture
```

The installer downloads the pinned CLI release into `.tools/bin`, verifies the
published SHA-256 checksum, and does not modify the system installation.

Run the read-only Temporal Server 1.29/1.30/1.31 matrix:

```sh
scripts/compatibility.sh
```

## Design

The application state machine is pure: keyboard events and async responses
produce typed commands, while the Tokio runtime executes those commands through
a small `TemporalService` boundary. Request IDs suppress stale list, detail,
mutation, and cross-cluster responses. Profile switching resolves secret
references lazily and swaps the active service only after the new Temporal
frontend responds successfully. Ratatui `TestBackend` tests cover the dashboard,
confirmation modals, profile switcher, and small-terminal fallback without
requiring a real terminal.

The Worker list/detail surface uses Temporal's experimental heartbeat API and
is shown as unavailable rather than guessed when a server or SDK does not
provide heartbeat data. Worker Deployment routing and drainage use the GA APIs
introduced in Temporal Server 1.31.
