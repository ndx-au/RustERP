# Nucleus Spec — Phase 1 Parties Domain

## Goal

Land the first **functional business domain** — **Parties** (customers, suppliers, prospects, and their contacts) — as a real `rusterp-parties` workspace crate with an honest domain model, in-memory persistence for tests, module-registry integration, and protobuf message definitions — without a live database driver stack or gRPC server.

## Constraints

- **License:** Apache-2.0; preserve root `LICENSE` / `NOTICE`.
- **Build:** Stable Rust; `cargo check` and `cargo test` at repo root must pass with no external services.
- **Architecture:** Single-tenant; API-first. Domain logic lives in `crates/rusterp-parties`. UIs remain out of scope.
- **Party model:** One **Party** entity may hold one or more **roles** among: `Customer`, `Supplier`, `Prospect` (not three disconnected aggregate roots).
- **Contacts:** Belong to a Party (person or named contact point); at least name + optional email/phone.
- **Persistence this phase:** Domain **repository trait** + **in-memory** implementation sufficient for unit/integration tests. Do **not** require real SQLite/Postgres drivers yet. May extend `rusterp-storage` only if needed for shared ID/error types — prefer keeping parties self-contained.
- **Module activation:** Parties is a registerable functional domain via `rusterp-modules` (not always-on unless Decision Log says otherwise — default **not** always-on).
- **Protobuf:** Add `proto/rusterp/party/v1/` messages (and optional service RPCs as comments or empty service) matching `rusterp.party.v1`. **No** tonic/prost codegen or network server required.
- **IDs:** Stable opaque string or UUID-as-string identifiers generated in-process; document choice in Decision Log when implementing.
- **Honesty:** No fake CRM pipeline, no credit scoring, no “synced to accounting” claims.
- **`.gitignore`:** Keep tracking `.nucleus/` except `attest.key` (accepted Phase 0 override). Continue ignoring `target/`.

## Acceptance Criteria

- [ ] Workspace member **`crates/rusterp-parties`** exists and is listed in root `Cargo.toml` members (still **no** catalog/sales/payments/inventory crates).
- [ ] Domain types cover at minimum:
  - `Party` (id, display name, roles set, timestamps or `created_at` stub field, active/archived flag or equivalent)
  - `PartyRole` enum: Customer | Supplier | Prospect
  - `Contact` (id, party_id, name, optional email, optional phone)
- [ ] **`PartyRepository`** (or equivalent) trait with create/get/list/update (or upsert) and attach/list contacts; **`InMemoryPartyRepository`** implements it.
- [ ] Unit tests prove:
  - create party with one or more roles and fetch by id
  - party can be both customer and supplier
  - add contact to party and list contacts for that party
  - unknown id returns a clear error
- [ ] Functional module id **`parties`** (exact string) can be registered and enabled through `rusterp-modules`; test or example wiring shows the link (parties crate may depend on `rusterp-modules` **or** a thin integration test/docs path — prefer a unit test in `rusterp-parties` or workspace doc test that constructs a registry and registers `parties`).
- [ ] **`proto/rusterp/party/v1/`** contains at least one `.proto` with `package rusterp.party.v1;` defining messages for Party, PartyRole, Contact (fields may be minimal). Codegen not required.
- [ ] Root **README** (and `AGENTS.md` repo map if present) mentions Parties as the first landed functional domain crate.
- [ ] **`cargo check`** exits 0 (attested).
- [ ] **`cargo test`** exits 0 (attested), including new parties tests.
- [ ] No gRPC server binary, no DB driver dependencies, no UI crates added.

## Out-of-Scope

- Real SQLite/PostgreSQL schemas, migrations, Litestream wiring.
- gRPC server, tonic/prost build scripts, network listeners.
- AuthN/AuthZ enforcement on party APIs (document RBAC resource names only if trivial, e.g. comments `party:read` — no enforcer).
- Catalog, Sales, Payments, Inventory domains.
- Addresses beyond a single optional free-text field if already needed for Contact/Party (full address book / multiple sites → later).
- Soft-delete workflows, merge-duplicate parties, GDPR export, portals, e-invoicing party IDs (GLN etc.) unless a single optional `external_ref` string is trivial.
- UI / Macaron / WASM client.
- Always-on Core Platform identity domain (Users/Groups) — separate from Parties.

## Decision Log / Open Questions

| Decision / Question | Status | Notes |
|---------------------|--------|-------|
| Phase 0 accepted | **decided** | Foundations in place; gitignore tracks `.nucleus/` except `attest.key` |
| Party = single aggregate with multi-role | **decided** | Customer ∩ Supplier allowed |
| Roles in MVP Parties | **decided** | Customer, Supplier, Prospect |
| Contacts child-of-party | **decided** | |
| Persistence Phase 1 = in-memory repo | **decided** | Real storage backends when platform storage matures |
| Module id `parties` | **decided** | Not always-on |
| Proto package `rusterp.party.v1` | **decided** | Matches proto README convention |
| ID type (UUID v4 string vs ulid vs sequential) | **open** | Implementer picks one; document in crate rustdoc; prefer UUID v4 string via `uuid` crate if dependency acceptable |
| Optional `organization` vs `person` party kind | **open** | If omitted, display name only is enough for Phase 1 |
| Depend on `rusterp-storage`? | **decided** | **No** required dependency this phase |
| Expand `Storage` trait for parties? | **decided** | **No** this phase |
| Service RPCs in `.proto` | **open** | Messages required; unary RPCs optional sketches |

---

When satisfied: `/spec approve` then `/implement`.
