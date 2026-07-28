# AGENTS.md — RustERP

Working contract for coding agents (and humans pairing with them) in this
repository.

**Product:** RustERP by NDX Pty Ltd — modular, snug-fit, open-source ERP in Rust
for SMBs.  
**Site:** https://RustERP.biz  
**Source:** https://github.com/ndx-video/RustERP  
**License:** Apache-2.0 — see [LICENSE](./LICENSE), [NOTICE](./NOTICE),
[CONTRIBUTING.md](./CONTRIBUTING.md).

## Non-negotiables

1. **Honesty over cleverness.** Do not claim tests, builds, or commands succeeded
   unless a real Nucleus attestation exists under `.nucleus/attestations/`.
2. **Spec first for non-trivial work.** Use the Nucleus loop: Spec → approve →
   Implement → Attest → Review → Accept. Spec path defaults to
   `.nucleus/specs/current.md`.
3. **Faithfulness to the approved Spec.** No scope expansion, no drive-by
   refactors, no “while I’m here” domain features.
4. **Apache-2.0 only** for first-party code. New files should be clearly
   project-owned; watch third-party license compatibility when adding crates.
5. **Single-tenant, API-first.** No multi-tenant shortcuts. UIs are separate
   gRPC consumers — do not smuggle server-side UI frameworks into platform crates
   without an explicit Spec.

## Locked architecture (do not quietly reverse)

| Topic | Decision |
|-------|----------|
| Tenancy | Single-tenant; self-host / LXC-container isolation preferred |
| API | Headless gRPC + protobufs (`proto/`, package style `rusterp.<area>.v1`) |
| Browser transport | **Dual-mode server:** TCP gRPC (`RUSTERP_LISTEN`, default `:50051`) for API tools; HTTP + **slozhn** gRPC-over-WebSocket (`RUSTERP_HTTP_LISTEN`, default `:8123`, path `/rpc`) for egui WASM UI |
| Reference UI | **Separate repo** [RustERP-UI-WASM](https://github.com/ndx-video/RustERP-UI-WASM) — consumer only; never embed UI in core crates |
| Server config | Optional `rusterp-server.toml` (`RUSTERP_CONFIG`): TCP/HTTP listen, `static_dir`, port-conflict policy |
| Port conflicts | Default **clobber**: restart prior self via pidfile, then free port; alternative **fail** |
| Storage | Traits over **SQLite + Litestream** (default/recommended) and **PostgreSQL** |
| Authz | Classic RBAC (Users / Groups / Roles / `resource:action`); OIDC later |
| Modularity | Snug-fit activation of **functional domains** via module registry |
| Crate naming | `rusterp-*` under `crates/` |

### Functional domains vs technical modules

- **Functional domains** — user-activatable business capability (Core, Parties,
  Catalog, Sales, Payments, Inventory, …). Registry: `rusterp-modules`.
- **Technical modules** — code/platform crates (storage, future auth, events,
  audit, numbering). Not end-user toggles.

MVP functional intent (not all implemented yet): always-on Core Platform;
Parties; Catalog; Sales (Quotes → Orders → Invoices + Credit Notes); Payments &
Banking; **toggleable** Inventory.  
**Out of MVP:** Purchasing, full double-entry GL, advanced CRM, projects, HR,
manufacturing.

## Repository map

```text
Cargo.toml                 workspace root
AGENTS.md                  this file
README.md                  human-facing mission + decisions
rusterp-server.toml.example  sample server config (listen + port policy)
CONTRIBUTING.md            DCO + contribution norms
install.sh                 bootstrap rusterp helper CLI only (not full ERP)
dist/
  deploy-ui-stack.sh       build RustERP-UI-WASM → dist/ui/ + run server
  ui/                      WASM static shell (built artifact; git may track smoke copies)
  rusterp, rusterp-lib.sh  installer helper CLI
  test-dist.sh             offline installer smoke
crates/
  rusterp-storage/         storage traits + backend stubs
  rusterp-modules/         functional module registry / activation
  rusterp-parties/         Parties domain (customers/suppliers/prospects/contacts)
  rusterp-proto/           tonic/prost codegen from proto/
  rusterp-server/          dual-transport binary (TCP gRPC + HTTP/slozhn + static WASM)
    src/config.rs          TOML config loader
    src/http.rs            axum + slozhn WebSocket /rpc + ServeDir static
    src/port_guard.rs      listen-port clobber/fail policy
proto/                     protobuf conventions and .proto sources
.local/                    runtime pid/log (gitignored; created at deploy/run)
.nucleus/                  Nucleus local state (gitignored entire tree)
.out/                      agent turn dumps (gitignored)
```

**Sibling checkout:** local dev expects RustERP-UI-WASM beside this repo
(`../RustERP-UI-WASM`) unless `RUSTERP_UI_ROOT` is set.

Do not invent new top-level product trees without updating this file and the
README. Prefer new work as `crates/rusterp-<name>` workspace members.

## Dual transport & reference UI (operational)

Macaron-aligned stack in `rusterp-server`:

```text
TCP  :50051  → tonic gRPC (PartyService, HealthService, reflection)
HTTP :8123   → GET /     ServeDir from static_dir (default dist/ui/)
            → WS  /rpc   slozhn gRPC-over-WebSocket (shared grpc routes)
```

- **Config precedence:** CLI → `rusterp-server.toml` → `RUSTERP_*` env → defaults.
- **HTTP default port:** `:8123` (not `:8080`).
- **`dist/ui/`:** output of `dist/deploy-ui-stack.sh` (trunk build from RustERP-UI-WASM).
  Do not hand-edit; rebuild and redeploy.
- **Deploy script:** `./dist/deploy-ui-stack.sh [--bg]` from repo root; sets
  `RUSTERP_CONFIG`, creates `.local/` for pid/log.
- **Reverse proxy pattern:** TLS at edge; UI host → `:8123`, API host → `:50051` h2c.
- **UI repo:** WASM uses glow renderer + slozhn client; graphics failures show a
  user-facing troubleshooting overlay (canvas permission / WebGL), not a bare panic.

## Build & verify

Stable Rust toolchain. From repo root:

```bash
cargo check
cargo test
```

When Nucleus is active, run verification through **`nucleus_attest`** (not bare
claims). Prefer small, reviewable diffs.

## Implementation norms

- **Thin stubs over fake completeness.** Prefer traits, registries, and empty-ish
  protos until a Spec calls for real behavior.
- **Domain crates land when the domain lands.** Do not add empty
  parties/catalog/sales/… crates “for later” unless the Spec says so.
- **Storage:** keep I/O behind `rusterp-storage` (or successors). Litestream is an
  ops path for SQLite, not a silent required runtime crate unless specified.
- **Protos:** definitions under `proto/`; codegen/server wiring only when Spec’d.
- **HTTP/UI transport:** keep axum/slozhn/static serving in `rusterp-server` only;
  do not pull egui/eframe into core crates. UI changes belong in RustERP-UI-WASM.
- **Shared gRPC routes:** TCP and slozhn legs must share the same service wiring
  (`build_grpc_routes` / in-memory repo) — no divergent handlers per transport.
- **Edition:** 2021 workspace default unless a Spec changes it.
- **Commits:** contributors use DCO (`Signed-off-by`) per CONTRIBUTING.md.

## Nucleus paths in this repo

| Path | Tracked? | Purpose |
|------|----------|---------|
| `.nucleus/` | **no** | Entire tree is local-only (specs, attestations, out, state, key) |
| `.out/` | **no** | Agent turn dumps outside Nucleus |

Both directories are **gitignored**. Keep them on disk for local honesty-loop
work; they must not appear in git history.

## Roles (when Nucleus is engaged)

| Role | Incentive |
|------|-----------|
| Planner | Clear, testable Spec; explicit out-of-scope |
| Implementer | Spec faithfulness + real attestation only |
| Adversarial Reviewer | Find fabrication, missing evidence, scope drift |

Stay in the assigned role. Do not mix Planner/Implementer/Reviewer incentives in
one turn.

## What “done” means

A change is done when:

1. Acceptance criteria in the approved Spec are met,
2. Required commands are **attested**,
3. Review has passed (or the human has explicitly overridden),
4. Docs that the Spec requires (README / AGENTS / proto notes) are updated.

If the Spec is ambiguous, **stop and ask** — do not invent product decisions.
