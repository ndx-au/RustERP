# Nucleus Spec — Phase 2: gRPC Bootstrap + Parties Service

## Goal

Make RustERP’s **API-first / headless** posture real: land a minimal, attest-able
**gRPC** surface that exposes the existing **Parties** domain (in-memory
repository) over protobuf/tonic — no new business domains, no UI, no real DB
drivers.

## Constraints

- **License:** Apache-2.0; preserve root `LICENSE` / `NOTICE`.
- **Build:** Stable Rust; `cargo check` and `cargo test` at repo root must pass
  with **no external services** required for the default test suite.
- **Architecture:** Single-tenant; headless gRPC only. UIs remain separate
  consumers (no server-side UI frameworks).
- **Domain source of truth:** Existing `rusterp-parties` model +
  `InMemoryPartyRepository` (or equivalent shared in-process state). Do **not**
  invent a parallel party model in the server crate.
- **Codegen:** `tonic` + `prost` (idiomatic current stack); protobuf sources stay
  under `proto/`; package style `rusterp.<area>.v1`.
- **Auth:** No AuthN/AuthZ enforcement. Optional comments / docs for future RBAC
  resource names only.
- **Persistence:** Still in-memory only. No SQLite/Postgres drivers, migrations,
  or Litestream.
- **Domains:** No Catalog, Sales, Payments, or Inventory crates or protos in this
  change.
- **Honesty:** Server is a thin wiring layer + local smoke path — not a
  production-hardened deployment.
- **Workspace norms:** New work as `crates/rusterp-*` members; keep diffs
  reviewable.

## Acceptance Criteria

- [ ] **`PartyService`** is defined under `proto/rusterp/party/v1/` (existing
      Party / PartyRole / Contact messages retained or extended only as needed)
      with at least these RPCs: `CreateParty`, `GetParty`, `ListParties`,
      `UpdateParty`, `AddContact`, `ListContacts` (request/response messages as
      needed).
- [ ] A minimal **Health** (or readiness) surface exists: either
      `proto/rusterp/platform/v1/` Health service **or** an equivalent tiny ping
      RPC — enough to prove the process is serving gRPC.
- [ ] **Codegen pipeline** builds generated Rust from the protos via tonic/prost
      (build.rs and/or a small dedicated codegen crate). Document how to
      regenerate in README (or crate-level docs).
- [ ] Workspace member **`crates/rusterp-server`** exists (binary) and is listed
      in root `Cargo.toml` members. It runs a thin gRPC server that wires
      **in-memory** Parties state to the generated `PartyService` impl.
- [ ] Default listen address is **`127.0.0.1:50051`**, overridable (CLI flag
      and/or env var — document the chosen mechanism).
- [ ] Server serves at least **CreateParty**, **GetParty**, and **ListParties**
      correctly against the in-memory repo (remaining PartyService RPCs
      implemented or clearly stubbed with honest errors — prefer full minimal
      impl of all six listed RPCs if small).
- [ ] At least **one automated test** covers service wiring (prefer tonic
      in-process / router test or library-level service test that does not
      require a free TCP port; if a bind-based test is used, it must be reliable
      in CI-like local runs).
- [ ] **No** new functional domain crates; **no** real DB driver dependencies
      added for application use.
- [ ] Root **README** (and `AGENTS.md` repo map if present) documents: build,
      run server, example `grpcurl` (or equivalent) calls for Health + at least
      one Parties RPC, and explicit notes that **persistence is in-memory** and
      **auth is not enforced**.
- [ ] **`cargo check`** exits 0 (attested).
- [ ] **`cargo test`** exits 0 (attested), including the new wiring test(s).
- [ ] Optional: gRPC **server reflection** enabled in the binary to ease
      `grpcurl` discovery (Decision Log default: **yes** if low-cost).

## Out-of-Scope

- Real SQLite / PostgreSQL schemas, migrations, connection pools, Litestream.
- TLS, mTLS, rate limiting, multi-listener production ops.
- AuthN/AuthZ, OIDC, API keys, sessions, multi-tenant routing.
- Catalog, Sales, Payments, Inventory (or any new functional domain).
- WASM / Macaron / any UI crate in this repo.
- Expanding Parties business rules (merge, GDPR, addresses book, CRM pipeline).
- Always-on Core identity domain (Users/Groups) beyond a tiny Health proto if
  placed under `platform.v1`.
- Guaranteeing a live network smoke in attestation if flaky; automated
  in-process test is the honesty bar. Manual `grpcurl` is documented, not
  required to be attested.

## Decision Log / Open Questions

| Decision / Question | Status | Notes |
|---------------------|--------|-------|
| Phase 1 Parties accepted | **decided** | In-memory domain crate is upstream of this phase |
| Server crate name | **decided** | `rusterp-server` under `crates/rusterp-server` (binary) |
| Codegen stack | **decided** | tonic + prost; protos remain source of truth under `proto/` |
| Health surface | **decided** | Separate `rusterp.platform.v1` Health (or Check) service — not bolted onto PartyService |
| Default listen | **decided** | `127.0.0.1:50051`; override via CLI and/or env (implementer documents exact flag/env names) |
| gRPC reflection | **decided** | **Enable** in the server binary for Phase 2 if dependency cost is modest; document `grpcurl` list/describe |
| Shared state | **decided** | Single in-process `InMemoryPartyRepository` (mutex/async mutex as needed) — not multi-tenant |
| Auth | **decided** | None this phase |
| Persistence | **decided** | In-memory only |
| Where generated code lives | **open** | Prefer small `rusterp-proto` (or `rusterp-grpc`) lib crate consumed by server — implementer picks one clear layout and documents it |
| Async runtime | **open** | tokio expected with tonic; confirm in impl docs |
| Exact override flag/env names | **open** | e.g. `--listen` / `RUSTERP_LISTEN` — implementer chooses and documents |
| Full impl vs stub for Update/Add/List contacts | **decided** | Implement all six PartyService RPCs minimally (map domain errors to tonic `Status`) |

---

When satisfied: `/spec approve` then `/implement`.
