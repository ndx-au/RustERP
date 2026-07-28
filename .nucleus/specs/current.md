# Nucleus Spec — Macaron transport (dual-mode server)

## Goal

Align `rusterp-server` with the updated [Macaron](https://github.com/ndx-video/macaron)
/Tokio architecture while preserving the existing **TCP gRPC API** for headless
consumers (`grpcurl`, Caddy h2c, integrations). Add a second **HTTP** listener
with **gRPC-over-WebSocket** (`slozhn` at `/rpc`) and optional static WASM shell
for browser UI consumers.

## Constraints

- **License:** Apache-2.0; no third-party license conflicts.
- **Tenancy:** Single-tenant unchanged.
- **Dual transport:** `RUSTERP_LISTEN` / `:50051` TCP gRPC **unchanged** (grpcurl smoke paths stay valid).
- **HTTP listen:** `RUSTERP_HTTP_LISTEN` default `127.0.0.1:8080`; CLI `--http-listen`.
- **Domain:** No Parties logic, auth, TLS, or persistence changes.
- **Deps:** Bump tonic/prost to **0.14**; add workspace `[workspace.dependencies]`.
- **Build:** Root `cargo check` / `cargo test` must stay green without external services.

## Acceptance Criteria

- [ ] TCP gRPC on `RUSTERP_LISTEN` (default `127.0.0.1:50051`) serves Party + Health + reflection as today.
- [ ] HTTP on `RUSTERP_HTTP_LISTEN` (default `127.0.0.1:8080`) serves:
  - `ws://…/rpc` slozhn gRPC-over-WebSocket for the same services.
  - Optional static assets from `dist/` when present (same-origin WASM).
- [ ] Shared service state (`SharedRepo`) backs both transports.
- [ ] HTTP listener uses graceful shutdown + root `CancellationToken` (Ctrl+C / SIGTERM).
- [ ] Workspace deps table; `rusterp-proto` and `rusterp-server` on tonic/prost 0.14.
- [ ] README documents dual ports; `proto/README.md` notes browser transport = slozhn.
- [ ] `AGENTS.md` locked decision row for dual transport.
- [ ] `cargo check` and `cargo test` attested at repo root.

## Out-of-Scope

- Auth, streaming/bidi RPCs, background generators, domain features.
- Installer / Phase 3 bootstrap changes.
- Removing TCP gRPC or changing default `:50051`.
- UI crate in this repo.

## Decision Log

| Decision | Status | Notes |
|----------|--------|-------|
| Transport shape | **decided** | Dual: TCP `:50051` + HTTP `:8080` slozhn |
| Static dir | **decided** | `dist/` at repo root; override `RUSTERP_STATIC` |
| tonic version | **decided** | 0.14 aligned with Macaron |

---

Approved for implementation.
