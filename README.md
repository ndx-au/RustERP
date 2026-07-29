# RustERP by NDX Pty Ltd

**Official site:** [https://RustERP.biz](https://RustERP.biz)

**RustERP** is a modular, snug-fit, open-source ERP written in Rust for
small-to-medium businesses. Users should feel the immediacy of classic desktop
apps; implementors should enable only what each business needs so the product
stays cognitively efficient for end users.

This repository is the open-source project source. Product information,
documentation, and project news live on the official site.

## Source & community

| | |
|--|--|
| **Website** | [RustERP.biz](https://RustERP.biz) |
| **Source** | [github.com/ndx-video/RustERP](https://github.com/ndx-video/RustERP) |
| **Contributing** | [CONTRIBUTING.md](./CONTRIBUTING.md) |

## Architecture decisions

| Decision | Choice |
|----------|--------|
| **License** | Apache-2.0 (see [LICENSE](./LICENSE) and [NOTICE](./NOTICE)) |
| **Tenancy** | Single-tenant by design; self-host with LXC/container isolation preferred |
| **API** | API-first / headless — **gRPC + protobufs**; UIs are separate consumers |
| **Storage** | **PostgreSQL** via sqlx (connection pool, migrations, health ping) |
| **Permissions** | Classic **RBAC** (Users / Groups / Roles / `resource:action`); SSO (OIDC) later |
| **Modularity** | “Snug fit” — consultants activate only the functional domains a business needs |

### Functional domains vs technical modules

- **Functional domains** — user-facing capabilities that can be activated per
  deployment (MVP intent: always-on Core Platform, Parties, Catalog, Sales,
  Payments & Banking, toggleable Inventory). Owned conceptually by the module
  registry (`rusterp-modules`).
- **Technical modules** — code organization (platform crates such as storage,
  future auth/RBAC/events). Not end-user toggles.

Purchasing, full double-entry GL, advanced CRM, projects, HR, and manufacturing
are out of MVP scope.

### Repository layout

```text
Cargo.toml                 workspace root
rusterp-server.toml        optional server config (copy from rusterp-server.toml.example)
install.sh                 bootstrap `rusterp` helper CLI only
dist/
  deploy-ui-stack.sh       build UI WASM + install to dist/ui/ + run server
  ui/                      static WASM shell (built artifact; not hand-edited)
  rusterp, rusterp-lib.sh  installer helper CLI
  test-dist.sh             offline installer smoke
crates/
  rusterp-storage/         PostgreSQL storage (sqlx pool + migrations)
  rusterp-modules/         functional module registry / activation skeleton
  rusterp-parties/         Parties domain (first functional domain)
  rusterp-proto/           tonic/prost codegen from proto/
  rusterp-server/          dual-transport server (TCP gRPC + HTTP/slozhn)
    src/config.rs          optional TOML config loader
    src/http.rs            axum static shell + slozhn WebSocket at /rpc
    src/port_guard.rs      listen-port conflict policy (default: clobber)
proto/                     gRPC/protobuf conventions and .proto defs
.local/                    runtime pid/log (gitignored; created by deploy)
```

**Parties** is the first landed functional domain crate (`rusterp-parties`):
customers, suppliers, prospects (multi-role party model) and contacts, with an
in-memory repository for tests. Catalog, Sales, Payments, and Inventory crates
are not present yet.

**Reference UI** lives in a **separate repository**:
[RustERP-UI-WASM](https://github.com/ndx-video/RustERP-UI-WASM) (egui/eframe
WASM + native client). It consumes the core over **slozhn** gRPC-over-WebSocket
at `/rpc` — never embed UI code in this repo.

**gRPC (Phase 2):** `rusterp-server` exposes `rusterp.party.v1.PartyService` and
`rusterp.platform.v1.HealthService` over the in-memory party store. **Persistence
is not durable** and **authentication is not enforced**. Generated types live in
`rusterp-proto` (sources under `proto/`).

### Dual transport (Macaron-aligned)

`rusterp-server` runs **two listeners** in one process (shared in-memory state):

```text
Browser / WASM UI ──► HTTP :8123 ──► GET /          static WASM (dist/ui/)
                                    WS  /rpc         slozhn gRPC-over-WebSocket

API tools / grpcurl ──► TCP :50051 ──► native gRPC + server reflection
```

| Leg | Default | Use |
|-----|---------|-----|
| TCP gRPC | `127.0.0.1:50051` (`RUSTERP_LISTEN`) | `grpcurl`, integrations, reverse-proxy h2c (e.g. Caddy) |
| HTTP + slozhn WS | `127.0.0.1:8123` (`RUSTERP_HTTP_LISTEN`) | Browser/native UI at `ws://host/rpc`; optional static WASM from `dist/ui/` |

Port **8123** is the deliberate HTTP default (avoids common dev ports like `:8080`).
Same-origin WASM uses page-relative `/rpc`; off-origin clients use
`ws://127.0.0.1:8123/rpc`.

Protobuf: `proto/rusterp/party/v1/` (`rusterp.party.v1`),
`proto/rusterp/platform/v1/` (`rusterp.platform.v1`).

## Development

Requires a stable Rust toolchain and **`protoc`** (Protocol Buffers compiler)
on `PATH` for `rusterp-proto` codegen.

```bash
cargo check
cargo test
```

No external database is required. Default tests do not need a running server.

### Run the server

```bash
cargo run -p rusterp-server
# override listen (CLI wins over config/env):
cargo run -p rusterp-server -- --listen 127.0.0.1:50051 --http-listen 127.0.0.1:8123
RUSTERP_LISTEN=127.0.0.1:50052 RUSTERP_HTTP_LISTEN=127.0.0.1:8123 cargo run -p rusterp-server
```

**Configuration precedence** (highest wins): CLI flags →
[`rusterp-server.toml`](./rusterp-server.toml) (or path in `RUSTERP_CONFIG`) →
`RUSTERP_*` env vars → built-in defaults.

Copy [`rusterp-server.toml.example`](./rusterp-server.toml.example) to
`rusterp-server.toml` to customize listen addresses, static WASM directory, and
port-conflict policy. Example:

```toml
[tcp]
listen = "0.0.0.0:50051"

[http]
listen = "0.0.0.0:8123"
static_dir = "dist/ui"

[port_conflict]
policy = "clobber"   # default: restart self via pidfile, then free the port
pid_file = ".local/rusterp-server.pid"
graceful_secs = 5
```

| Variable | Default | Purpose |
|----------|---------|---------|
| `RUSTERP_CONFIG` | `rusterp-server.toml` in cwd / repo root | Path to server TOML |
| `RUSTERP_HOME` | cwd | Prefix for relative `pid_file` paths |
| `RUSTERP_LISTEN` | `127.0.0.1:50051` | TCP gRPC (grpcurl, API tools) |
| `RUSTERP_HTTP_LISTEN` | `127.0.0.1:8123` | HTTP + gRPC-over-WebSocket at `/rpc` |
| `RUSTERP_STATIC` | from config or `dist/ui/` when present | Static WASM shell directory |
| `RUSTERP_POSTGRES_URL` | _(required)_ | PostgreSQL connection URI |
| `RUSTERP_PG_MAX_CONNECTIONS` | `20` | sqlx pool max connections |
| `RUSTERP_PG_MIN_CONNECTIONS` | `2` | sqlx pool min warm connections |
| `RUSTERP_PG_ACQUIRE_TIMEOUT_SECS` | `3` | Pool acquire timeout (seconds) |

On startup, if a listen port is busy the server applies `[port_conflict]`:
**clobber** (default) sends SIGTERM to the prior instance recorded in the
pidfile, waits, then kills any remaining occupant; **fail** exits with an error.

Server reflection is enabled on the TCP leg for discovery tools. Both listeners
shut down together on Ctrl+C / SIGTERM.

### Storage (PostgreSQL)

`rusterp-server` requires a PostgreSQL connection URI via `RUSTERP_POSTGRES_URL`
or `[storage].postgres_url` in `rusterp-server.toml`. Persistence uses
[`sqlx`](https://github.com/launchbadge/sqlx) with an explicitly tuned
connection pool and embedded migrations applied at startup.

| Setting | Default | Purpose |
|---------|---------|---------|
| `max_connections` | 20 | Pool size cap |
| `min_connections` | 2 | Warm connections kept open |
| `acquire_timeout_secs` | 3 | Fail fast when pool is exhausted |
| `idle_timeout_secs` | 600 | Recycle idle connections |
| `max_lifetime_secs` | 1800 | Recycle connections before LB/firewall drops |

Health checks call `SELECT 1` on the pool. Local dev and tests that need a
real database require a running PostgreSQL instance; unit tests use
in-memory repositories and skip integration tests when `RUSTERP_POSTGRES_URL`
is unset.

### Reference UI (separate repo)

The egui WASM/native shell is **[RustERP-UI-WASM](https://github.com/ndx-video/RustERP-UI-WASM)**.
It talks to this core over slozhn at `/rpc` (default `ws://127.0.0.1:8123/rpc` for
native dev). Privacy browsers may block WebGL/canvas until the user allows it
via the address-bar permission icon — the UI shows a Macaron-style troubleshooting
card when graphics init fails.

### Deploy UI stack (WASM + server)

From this repo root, with a sibling checkout of RustERP-UI-WASM (or set
`RUSTERP_UI_ROOT`):

```bash
./dist/deploy-ui-stack.sh          # trunk build + rsync to dist/ui/ + foreground server
./dist/deploy-ui-stack.sh --bg     # same, server in background
```

Requires **`trunk`**, **`protoc`**, and **`wasm32-unknown-unknown`**. The script
builds the UI, copies artifacts to `dist/ui/`, builds `rusterp-server` release,
and starts with `RUSTERP_CONFIG` (default `./rusterp-server.toml`). Background
mode logs to `.local/rusterp-server.log`.

Typical reverse-proxy layout (TLS terminates at the proxy; host listens on
`:8123` HTTP and `:50051` TCP):

| Public host | Upstream | Purpose |
|-------------|----------|---------|
| `rusterp-ui.example` | `host:8123` | WASM shell + slozhn `/rpc` |
| `rusterp-api.example` | `host:50051` h2c | TCP gRPC for API tools |

### Example `grpcurl` (optional manual check)

With [grpcurl](https://github.com/fullstorydev/grpcurl) and the server running:

```bash
# list services (reflection)
grpcurl -plaintext 127.0.0.1:50051 list

# health
grpcurl -plaintext 127.0.0.1:50051 rusterp.platform.v1.HealthService/Check

# create a party
grpcurl -plaintext -d '{
  "display_name": "Acme Ltd",
  "roles": ["PARTY_ROLE_CUSTOMER"]
}' 127.0.0.1:50051 rusterp.party.v1.PartyService/CreateParty

# list parties
grpcurl -plaintext -d '{}' 127.0.0.1:50051 rusterp.party.v1.PartyService/ListParties
```

### Regenerate protobuf Rust

```bash
cargo build -p rusterp-proto
```

See [proto/README.md](./proto/README.md).

## Install core (source bootstrap)

Self-host / LXC path for the **core** server (single-tenant). **Source
install** — needs **git**, **stable Rust (`cargo`)**, and **`protoc`**. Does **not**
auto-install toolchains. The reference UI ([RustERP-UI-WASM](https://github.com/ndx-video/RustERP-UI-WASM))
is a separate checkout; use [`dist/deploy-ui-stack.sh`](./dist/deploy-ui-stack.sh)
or serve WASM into `dist/ui/` manually.

### 1. Bootstrap the helper CLI

Linux and macOS. Installs only `rusterp` under a user-local prefix by default
(`~/.local/share/rusterp/cli` + `~/.local/bin/rusterp`). As root, defaults are
`/opt/rusterp` and `/usr/local/bin`. Re-run is safe (idempotent copy/symlink).

```bash
# from a checkout:
./install.sh

# or remote bootstrap of the CLI only:
curl -fsSL https://raw.githubusercontent.com/ndx-video/RustERP/main/install.sh | bash
```

Ensure `~/.local/bin` is on `PATH` if needed.

### 2. Install core from source

```bash
rusterp install core              # default git branch: dist
rusterp install core --branch dist
rusterp install core --branch my-pin
rusterp status
```

- Default ref is **`dist`**. There is **no silent fallback to `main`** if the
  branch is missing or unfetchable (fail-closed).
- Clone/build prefix: **`RUSTERP_HOME`** (default `~/.local/share/rusterp`, or
  `/opt/rusterp` when root).
- Builds `rusterp-server` in **release** mode and installs:
  - `$RUSTERP_HOME/bin/rusterp-server`
  - `$RUSTERP_HOME/bin/run-core` (wrapper honoring `RUSTERP_LISTEN`)
  - example env file and optional systemd unit template under `$RUSTERP_HOME`

### 3. Run core

```bash
# default listen 127.0.0.1:50051 (TCP gRPC only via run-core wrapper)
rusterp-run-core
# or run the server binary directly (both TCP + HTTP legs when configured):
"$HOME/.local/share/rusterp/bin/rusterp-server"
RUSTERP_CONFIG="$HOME/.local/share/rusterp/rusterp-server.toml" \
  RUSTERP_LISTEN=0.0.0.0:50051 \
  RUSTERP_HTTP_LISTEN=0.0.0.0:8123 \
  "$HOME/.local/share/rusterp/bin/rusterp-server"
```

The **`run-core`** wrapper honors `RUSTERP_LISTEN` for the TCP leg only. For
the HTTP/slozhn leg and static WASM, run `rusterp-server` with
`rusterp-server.toml` (see [Run the server](#run-the-server) above) or set
`RUSTERP_HTTP_LISTEN`.

**Persistence is in-memory. Authentication is not enforced.** Point the separate
[RustERP-UI-WASM](https://github.com/ndx-video/RustERP-UI-WASM) client or
`grpcurl` at the listen addresses when ready.

### Terraform-oriented environment

| Variable | Purpose |
|----------|---------|
| `RUSTERP_HOME` | Install prefix (source tree, binaries, state) |
| `RUSTERP_BIN_DIR` | Where `install.sh` places `rusterp` on `PATH` |
| `RUSTERP_REPO_URL` | Git remote for `install core` (default GitHub origin) |
| `RUSTERP_CONFIG` | Path to `rusterp-server.toml` (listen + port policy) |
| `RUSTERP_LISTEN` | TCP gRPC listen (`host:port`, default `127.0.0.1:50051`) |
| `RUSTERP_HTTP_LISTEN` | HTTP + slozhn listen (`host:port`, default `127.0.0.1:8123`) |
| `RUSTERP_BOOTSTRAP_REF` | Ref used when `install.sh` downloads CLI files remotely (default `main`) |
| `RUSTERP_UI_ROOT` | Sibling path to RustERP-UI-WASM for `deploy-ui-stack.sh` |

No interactive prompts. Prefer env or an `EnvironmentFile` (see
`dist/rusterp-server.service.example`).

### Offline checks / manual smoke

```bash
# no network — syntax + default-branch / argv parsing
./dist/test-dist.sh
```

Manual smoke (not required in CI): clean LXC → `./install.sh` →
`rusterp install core` → `rusterp status` → `run-core` → Health via `grpcurl`.

Design notes (non-normative): [DIST.md](./DIST.md).

## License

**RustERP by NDX Pty Ltd** is licensed under the **Apache License, Version 2.0**.
See [LICENSE](./LICENSE) and [NOTICE](./NOTICE).

Copyright 2026 NDX Pty Ltd and contributors.

You are free to use, modify, and redistribute RustERP — including in commercial
and internal products — under those terms. Contributions are welcome under the
same license; see [CONTRIBUTING.md](./CONTRIBUTING.md).
