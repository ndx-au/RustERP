# RustERP product roadmap

Phased plan for landing MVP domains. **Principle:** one vertical slice at a
time (schema → repository → proto → server → UI). Nucleus is legacy — agree
scope with the human and verify with real `cargo test` / deploy.

## Where we are

| Layer | Status |
|-------|--------|
| Platform | Dual transport, Postgres/sqlx migrations, deploy stack |
| Schema | Full MVP + post-MVP stubs — see [schema.md](./schema.md) |
| Domain crates | `rusterp-parties` live; others land per phase below |
| gRPC | `PartyService` + `HealthService`; auth not enforced yet |
| UI | Full IA shell; live Parties list + Settings Connection; other pages wireframe until their phase |

## Phases

| Phase | Outcome |
|-------|---------|
| **0** | Docs match Postgres-backed Parties |
| **1** | Parties CRUD + role filters + contacts + addresses in UI |
| **2** | Catalog products/categories live |
| **3** | Sales quotes → orders → invoices usable |
| **4** | Payments + allocations against invoices |
| **5** | Auth/RBAC + live `core.modules` toggles |
| **6** | Toggleable inventory |

## Cross-cutting rules

1. Core proto first, then vendor into UI client, then UI screens.
2. Empty DB = empty UI; no invented demo rows.
3. Single-tenant; no `tenant_id`.
4. Inventory stays optional via `core.modules`.
5. Post-MVP domains stay rail stubs only until an explicit product decision.

## Out of MVP

Purchasing, full double-entry GL, CRM, Projects, HR, Manufacturing.
