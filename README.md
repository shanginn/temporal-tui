# temporal-tui

`temporal-tui` is a keyboard-first Temporal dashboard and control plane built
with Rust and [Ratatui](https://ratatui.rs/). It connects directly to a Temporal
frontend through the official Rust client; the web UI and Temporal CLI are not
required at runtime.

## Capabilities

- Browse workflow visibility results with any Temporal visibility query.
- Inspect workflow metadata, pending work, and the latest 200 history events.
- Discover and switch namespaces without reconnecting.
- Refresh manually or automatically.
- Send a named signal with JSON input.
- Request graceful cancellation or terminate an exact workflow run.
- Connect to a local cluster, self-hosted Temporal, or Temporal Cloud with API
  key, TLS, mTLS, custom CA, server-name override, and repeated gRPC headers.
- Restore raw mode, the cursor, and the alternate screen on normal and error
  exits.

Cancellation and termination always require an explicit `y` confirmation.
Mutation commands retain both the workflow ID and run ID selected when the
confirmation opened, so a refresh cannot redirect an action to a different run.

## Build

The repository pins Rust 1.97.1, including Rustfmt and Clippy:

```sh
rustup show
cargo build --release --locked
./target/release/temporal-tui --help
```

The release binary is `target/release/temporal-tui`.
On macOS, the Xcode Command Line Tools must be installed and their license
accepted so Rust can invoke the system SDK and linker.

Install directly from GitHub:

```sh
cargo install --locked --git https://github.com/shanginn/temporal-tui
```

This initial version pins Ratatui 0.30.2, Crossterm 0.29.0,
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

## Keyboard controls

| Key | Action |
| --- | --- |
| `j` / `k`, arrows | Move through workflows or history |
| `g` / `G`, home / end | Jump to first or last item |
| `tab` / `enter` | Switch between workflow and history panes |
| `/` | Edit the Temporal visibility query |
| `n` | Switch namespace |
| `r` | Refresh now |
| `a` | Toggle automatic refresh |
| `s` | Send a signal with JSON input |
| `c` | Request workflow cancellation |
| `x` | Terminate a workflow |
| `?` | Open keyboard help |
| `q` / `ctrl-c` | Quit |

## Tests

Run the complete static, unit, UI-rendering, and release-build gate:

```sh
scripts/check.sh
```

The live contract test starts an ephemeral Temporal dev server and verifies
cluster discovery, namespaces, visibility, describe/history, signal, cancel,
and terminate through the real gRPC adapter:

```sh
scripts/install-temporal-cli.sh
cargo test --locked --test live_temporal -- --ignored --nocapture
```

The installer downloads the pinned CLI release into `.tools/bin`, verifies the
published SHA-256 checksum, and does not modify the system installation.

## Design

The application state machine is pure: keyboard events and async responses
produce typed commands, while the Tokio runtime executes those commands through
a small `TemporalService` boundary. Request IDs suppress stale list, detail, and
mutation responses. Ratatui `TestBackend` tests cover the dashboard, confirmation
modal, and small-terminal fallback without requiring a real terminal.
