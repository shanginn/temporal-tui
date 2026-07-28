# temporal-tui roadmap

The goal is a production-ready, keyboard-first Temporal operations console with
safe mutation controls, first-class encrypted payload support, and live
compatibility tests. Milestones are cut only after the locked quality gate and
the disposable-server contract both pass.

## Delivered

### v0.1 — working foundation

- Ratatui dashboard, Temporal connection, Workflow visibility, details, history,
  signal/cancel/terminate controls, terminal restoration, and initial tests.

### v0.2 — production Workflow operations

- Named connection profiles, TLS/mTLS/API-key/keyring support, saved queries,
  namespace switching, cursor pagination and aggregations.
- Full history paging, Workflow chains, pending Activities, failure trees, memo,
  Search Attributes, payload inspection/redaction, exports, clipboard, and Web
  UI links.
- Exact-target mutation confirmations and enforced read-only mode.

### v0.3 — Worker observability and payload safety

- Task Queue backlog, pollers, rates, limits, and deployment routing.
- Heartbeat Worker resource/slot/task/cache diagnostics.
- GA Worker Deployment inventory, routing propagation, and drainage state.
- Standard remote Codec Server encode/decode protocol with namespace routing,
  secret headers, time/size limits, and redirect blocking.

### v0.4 — Workflow messages and Schedule control

- Workflow Query and Update invocation with explicit zero/multiple JSON
  arguments, decoded outcomes, and Update failure rendering.
- Workflow pause/unpause and reset-at-event control.
- Schedule visibility, details, create, conflict-safe update, pause/unpause,
  trigger, backfill, and deletion.
- Live Rust SDK Worker coverage for Query, Update, pause, unpause, and reset;
  disposable-server coverage for the complete Schedule and Codec lifecycle.

### v0.5 — fleet operations

- Switch between configured clusters inside the TUI with a complete state reset
  and no cross-cluster stale responses.
- Inspect the namespace Search Attribute registry and manage custom attributes
  with explicit type and confirmation controls.
- Preview, start, inspect, stop, and terminate server-side batch operations
  without expanding target queries client-side.
- Set Worker Deployment current/ramping versions, ramp percentages, and
  promotion state with drainage-aware warnings.

### v1.0 — production release

- Negotiated capability evidence and independent degradation for unavailable,
  restricted, and transiently unknown APIs.
- Disposable read-only contracts for Server 1.29/1.30/1.31 plus the complete
  1.31 mutation contract.
- Atomic schema-1 to schema-2 migration with byte-identical private backup and
  persisted UI defaults.
- Linux, macOS ARM/Intel, and Windows release archives, completions, manpage,
  Homebrew/Scoop metadata, SHA-256, CycloneDX SBOM, and provenance.
- Cross-platform quality/live/compatibility CI and weekly
  advisory/license/source policy.
- Threat model, operations, troubleshooting, accessibility, and automated
  clean install/upgrade/uninstall verification.

## Next

### v1.1 — operator ergonomics

- User-defined column layouts and compact saved dashboard presets.
- Optional structured tracing export with explicit redaction policy.
- Additional Temporal Cloud authorization test fixtures.

### v1.2 — incident workflows

- Cross-view bookmarks for Workflows, Task Queues, Deployments, and Batch jobs.
- Compare two Workflow histories or Worker Deployment routing snapshots.
- Signed diagnostic bundles with operator-supplied incident metadata.

## Release invariants

- `master` is the only default and release branch.
- Every mutation is blocked by read-only mode and identifies its exact
  namespace and target.
- Destructive or broad operations show a preview and require typed
  confirmation.
- Secrets never enter config files, diagnostic exports, fixtures, logs, or Git.
- Tests never use or mutate a user-owned Temporal server.
- A release requires formatting, strict Clippy, unit/state/UI tests, optimized
  locked build, disposable Temporal live contract, and PTY terminal-restoration
  smoke test.
