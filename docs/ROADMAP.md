# RustERP product roadmap

Phased plan for landing MVP domains. **Principle:** one vertical slice at a
time (schema → repository → proto → server → UI). Nucleus is legacy — agree
scope with the human and verify with real `cargo test` / deploy.

## Where we are

| Layer | Status |
|-------|--------|
| Platform | Dual transport, Postgres/sqlx migrations, deploy stack |
| Schema | Full MVP + post-MVP stubs — see [schema.md](./schema.md) |
| Domain crates | `rusterp-parties`, `rusterp-catalog`, `rusterp-sales`, `rusterp-payments`, `rusterp-inventory`, `rusterp-auth` |
| gRPC | Party, Catalog, Sales, Payment, Inventory, Module, Auth, Health — soft RBAC via `RUSTERP_AUTH_ENFORCE` |
| UI | Live: Parties, Catalog (products/categories), Sales docs, Payments, Inventory (when enabled), Settings Modules & Users |

## Phases

| Phase | Outcome | Status |
|-------|---------|--------|
| **0** | Docs match Postgres-backed Parties | done |
| **1** | Parties CRUD + role filters + contacts + addresses in UI | done |
| **2** | Catalog products/categories live | done |
| **3** | Sales quotes → orders → invoices usable | done |
| **4** | Payments + allocations against invoices | done |
| **5** | Auth/RBAC + live `core.modules` toggles | done |
| **6** | Toggleable inventory | done |

## Cross-cutting rules

1. Core proto first, then vendor into UI client, then UI screens.
2. Empty DB = empty UI; no invented demo rows.
3. Single-tenant; no `tenant_id`.
4. Inventory stays optional via `core.modules`.
5. Post-MVP domains stay rail stubs only until an explicit product decision.

## Out of MVP

Purchasing, full double-entry GL, CRM, Projects, HR, Manufacturing.
