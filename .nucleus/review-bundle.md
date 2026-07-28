NUCLEUS — ISOLATED ADVERSARIAL REVIEW

Clean session: **no Implementer chat history**. Work only from the Review Bundle.
Ignore invented narrative. Prefer FAIL on exit_mismatch.

Contents: 1 Spec · 2 Diff · 3 Verified attestations · 4 Independent re-exec (MATCH/MISMATCH)


⚠ Re-exec **MISMATCH** already in §4 — default to FAIL unless explained.

--- BEGIN REVIEW BUNDLE ---

# Nucleus Review Bundle

Working set only: Spec · Diff · verified Attestation · re-exec. No Implementer chat.

## At a glance

| | |
|--|--|
| Change | chg-2026-07-27T15-30-13-948Z-afc57a |
| Phase | Reviewing |
| Spec | .nucleus/specs/current.md ✓ |
| Diff | present |
| Verified attestations | 3 |
| Re-exec | **MISMATCH** |

---

## 1. Spec

Path: `.nucleus/specs/current.md`

# Nucleus Spec — Phase 3: Distribution bootstrap (`install.sh` + `rusterp install core`)

## Goal

Give self-hosters and Terraform a **snug-fit, source-first** path to install the
RustERP **core** role: a thin root `install.sh` that bootstraps a small `rusterp`
helper CLI, plus `rusterp install core [--branch <name>]` that fetches this repo
at a chosen ref (default **`dist`**), builds `rusterp-server` in release mode, and
installs/links it under a documented prefix — no UI role, no binary packaging
monorepo.

## Constraints

- **License:** Apache-2.0; preserve root `LICENSE` / `NOTICE` / NDX identity.
- **Tenancy:** Single-tenant; one core instance per host/LXC is the isolation model.
- **Source-first:** git + cargo only this phase. No prebuilt release pipeline.
- **Roles, not monolith:** Implement **core** only. No `ui-wasm` installer.
- **Terraform-friendly:** env/file-oriented config; idempotent where practical;
  **no required interactive prompts**.
- **Track installer in this repo:** `install.sh` (and helper) live here;
  rusterp.biz may mirror later — not a separate packaging monorepo.
- **Platforms:** Linux + macOS for bootstrap scripts.
- **Server surface:** Do not change Parties/domain logic, auth, TLS, or multi-tenant
  wiring. Default listen remains aligned with `rusterp-server`
  (`127.0.0.1:50051` / `RUSTERP_LISTEN`).
- **Toolchains:** Fail honestly if `git` or `cargo` missing; **do not** auto-install
  rustup/cargo.
- **Honesty:** Prefer a working core source-install path over a generic package
  framework or theatrical “full product” installer.
- **Build:** If any Rust is added, root `cargo check` / `cargo test` must stay green
  with no external services. Shell-only is acceptable for Phase 3.

## Acceptance Criteria

- [ ] **`install.sh`** exists at **repo root**, documented for **Linux and macOS**.
      It bootstraps the `rusterp` helper into a conventional path (user-local by
      default for non-root; document any system path). Re-run is **safe/idempotent**
      enough for operator retry. It installs **only** the CLI/helper — not the whole
      ERP — and points operators at `rusterp install core`.
- [ ] **`rusterp` CLI** (shell OK) supports at least:
      - `rusterp install core [--branch <name>]`
      - `rusterp status` (minimal: core present?, binary path, listen hint if known)
- [ ] **Default branch** for `install core` is **`dist`** when `--branch` is omitted.
- [ ] **`--branch <name>`** overrides the git ref used for fetch/build.
- [ ] **Install behavior** (documented + implemented): clone/fetch this repo at the
      chosen ref into a documented prefix; `cargo build -p rusterp-server --release`;
      install or link the binary + a minimal run script (and optional example unit).
- [ ] **Missing `git` or `cargo`:** non-zero exit and a clear message with next steps
      (no silent toolchain install).
- [ ] **Missing / unfetchable branch:** fail closed (non-zero); do **not** silently
      fall back to `main` unless the Spec is later amended — Phase 3 default is
      **fail-closed** on bad/missing ref.
- [ ] **Config/run docs:** `RUSTERP_LISTEN` (and any data-dir or prefix env vars
      introduced) listed for Terraform; how to run core after install is clear.
- [ ] **README** section: bootstrap (`install.sh`) → `rusterp install core` → run
      server → optional note that UI is separate and points at the gRPC endpoint.
      States: source install needs **Rust stable + git**; default branch **`dist`**;
      variants via `--branch`.
- [ ] **No** `rusterp install ui-wasm` (or other roles) implemented in this change.
- [ ] **Automated checks** suitable for attestation without requiring a live git
      network install when possible, e.g.:
      - shell syntax (`bash -n` / `sh -n`) on `install.sh` and `rusterp` helper
      - unit-style tests or pure functions for default-branch / argv parsing if
        factored out
      - `cargo check` and `cargo test` at repo root still exit 0 (attested) if the
        workspace is touched or unchanged
- [ ] Manual smoke path **documented** (not required attested): clean machine/LXC
      `install.sh` → `rusterp install core` (or a documented `--dry-run` if added).

## Out-of-Scope

- `rusterp install ui-wasm` (UI repo / later Spec).
- Prebuilt binaries, deb/rpm, Homebrew, container images, Litestream sidecar packs.
- Production systemd/launchd hardening beyond a **minimal** example unit or run
  script.
- Auto-install of rustup/cargo/protoc on the target.
- SaaS / multi-tenant installer paths.
- AuthN/AuthZ, TLS, real DB drivers, domain feature work.
- Creating or maintaining the remote **`dist`** branch contents as part of this
  change (operators/CI may pin it; installer must use the name as default).
- Generic multi-role package framework beyond what core needs.

## Decision Log / Open Questions

| Decision / Question | Status | Notes |
|---------------------|--------|-------|
| Phase scope | **decided** | Bootstrap + `install core` only; source-first |
| Default git ref | **decided** | `dist` when `--branch` omitted |
| Missing branch policy | **decided** | Fail-closed; no silent `main` fallback |
| CLI implementation | **decided** | Shell acceptable for Phase 3 |
| Install prefix default | **decided** | **User-local** default (e.g. `~/.local/share/rusterp` + `~/.local/bin`); document optional root/`/opt/rusterp` path if supported |
| Bootstrap path for `rusterp` | **decided** | Prefer `~/.local/bin` (or equivalent) for non-root; document |
| systemd/launchd | **decided** | Minimal example unit **or** run script + docs only — implementer picks leanest honest option |
| `dist` branch existence in remote | **open / ops** | Installer defaults to name `dist`; creating the branch is outside this Spec’s code AC |
| Exact env names beyond `RUSTERP_LISTEN` | **open** | Implementer may add e.g. `RUSTERP_HOME` / prefix vars — must document in README |
| Dry-run flag | **open** | Optional; nice for CI/docs; not required if parsing tests cover defaults |
| protoc on target for source build | **open** | `rusterp-server` build may need `protoc`; document as prerequisite if still required — do not auto-install |

---

When satisfied: `/spec approve` then `/implement`.


---

## 2. Diff (git)

```diff
diff --git a/.nucleus/history.jsonl b/.nucleus/history.jsonl
index 97ca34b..239a48b 100644
--- a/.nucleus/history.jsonl
+++ b/.nucleus/history.jsonl
@@ -20,3 +20,13 @@
 {"ts":"2026-07-27T15:18:02.760Z","changeId":"chg-2026-07-27T15-00-17-641Z-bb3fef","phase":"Attested","event":"attestation","note":"recorded att-2026-07-27T15-18-02-756Z-4bdbaef6","attestationId":"att-2026-07-27T15-18-02-756Z-4bdbaef6"}
 {"ts":"2026-07-27T15:18:06.239Z","changeId":"chg-2026-07-27T15-00-17-641Z-bb3fef","phase":"Attested","event":"attestation","note":"recorded att-2026-07-27T15-18-06-237Z-6d9beb12","attestationId":"att-2026-07-27T15-18-06-237Z-6d9beb12"}
 {"ts":"2026-07-27T15:19:24.916Z","changeId":"chg-2026-07-27T15-00-17-641Z-bb3fef","phase":"Reviewing","event":"transition","note":"adversarial review started (Phase 2.0 isolation)","fromPhase":"Attested","toPhase":"Reviewing"}
+{"ts":"2026-07-27T15:23:19.098Z","changeId":"chg-2026-07-27T15-00-17-641Z-bb3fef","phase":"Accepted","event":"transition","note":"review pass","fromPhase":"Reviewing","toPhase":"Accepted"}
+{"ts":"2026-07-27T15:30:13.948Z","changeId":null,"phase":"idle","event":"transition","note":"new change after terminal phase","fromPhase":"Accepted","toPhase":"idle"}
+{"ts":"2026-07-27T15:30:13.949Z","changeId":"chg-2026-07-27T15-30-13-948Z-afc57a","phase":"SpecDraft","event":"change_boundary","note":"new change (previous: none)"}
+{"ts":"2026-07-27T15:30:13.949Z","changeId":"chg-2026-07-27T15-30-13-948Z-afc57a","phase":"SpecDraft","event":"transition","note":"spec draft started","fromPhase":"idle","toPhase":"SpecDraft"}
+{"ts":"2026-07-27T16:06:17.243Z","changeId":"chg-2026-07-27T15-30-13-948Z-afc57a","phase":"SpecApproved","event":"transition","note":"human approved spec","fromPhase":"SpecDraft","toPhase":"SpecApproved"}
+{"ts":"2026-07-27T16:06:24.228Z","changeId":"chg-2026-07-27T15-30-13-948Z-afc57a","phase":"Implementing","event":"transition","note":"implement started","fromPhase":"SpecApproved","toPhase":"Implementing"}
+{"ts":"2026-07-27T16:10:08.674Z","changeId":"chg-2026-07-27T15-30-13-948Z-afc57a","phase":"Attested","event":"attestation","note":"recorded att-2026-07-27T16-10-08-671Z-ffdece8e","attestationId":"att-2026-07-27T16-10-08-671Z-ffdece8e"}
+{"ts":"2026-07-27T16:10:09.310Z","changeId":"chg-2026-07-27T15-30-13-948Z-afc57a","phase":"Attested","event":"attestation","note":"recorded att-2026-07-27T16-10-09-308Z-3dbdb875","attestationId":"att-2026-07-27T16-10-09-308Z-3dbdb875"}
+{"ts":"2026-07-27T16:10:10.013Z","changeId":"chg-2026-07-27T15-30-13-948Z-afc57a","phase":"Attested","event":"attestation","note":"recorded att-2026-07-27T16-10-10-011Z-49674442","attestationId":"att-2026-07-27T16-10-10-011Z-49674442"}
+{"ts":"2026-07-27T16:11:09.124Z","changeId":"chg-2026-07-27T15-30-13-948Z-afc57a","phase":"Reviewing","event":"transition","note":"adversarial review started (Phase 2.0 isolation)","fromPhase":"Attested","toPhase":"Reviewing"}
diff --git a/.nucleus/specs/current.md b/.nucleus/specs/current.md
index 8b413dd..02081f5 100644
--- a/.nucleus/specs/current.md
+++ b/.nucleus/specs/current.md
@@ -1,103 +1,100 @@
-# Nucleus Spec — Phase 2: gRPC Bootstrap + Parties Service
+# Nucleus Spec — Phase 3: Distribution bootstrap (`install.sh` + `rusterp install core`)
 
 ## Goal
 
-Make RustERP’s **API-first / headless** posture real: land a minimal, attest-able
-**gRPC** surface that exposes the existing **Parties** domain (in-memory
-repository) over protobuf/tonic — no new business domains, no UI, no real DB
-drivers.
+Give self-hosters and Terraform a **snug-fit, source-first** path to install the
+RustERP **core** role: a thin root `install.sh` that bootstraps a small `rusterp`
+helper CLI, plus `rusterp install core [--branch <name>]` that fetches this repo
+at a chosen ref (default **`dist`**), builds `rusterp-server` in release mode, and
+installs/links it under a documented prefix — no UI role, no binary packaging
+monorepo.
 
 ## Constraints
 
-- **License:** Apache-2.0; preserve root `LICENSE` / `NOTICE`.
-- **Build:** Stable Rust; `cargo check` and `cargo test` at repo root must pass
-  with **no external services** required for the default test suite.
-- **Architecture:** Single-tenant; headless gRPC only. UIs remain separate
-  consumers (no server-side UI frameworks).
-- **Domain source of truth:** Existing `rusterp-parties` model +
-  `InMemoryPartyRepository` (or equivalent shared in-process state). Do **not**
-  invent a parallel party model in the server crate.
-- **Codegen:** `tonic` + `prost` (idiomatic current stack); protobuf sources stay
-  under `proto/`; package style `rusterp.<area>.v1`.
-- **Auth:** No AuthN/AuthZ enforcement. Optional comments / docs for future RBAC
-  resource names only.
-- **Persistence:** Still in-memory only. No SQLite/Postgres drivers, migrations,
-  or Litestream.
-- **Domains:** No Catalog, Sales, Payments, or Inventory crates or protos in this
-  change.
-- **Honesty:** Server is a thin wiring layer + local smoke path — not a
-  production-hardened deployment.
-- **Workspace norms:** New work as `crates/rusterp-*` members; keep diffs
-  reviewable.
+- **License:** Apache-2.0; preserve root `LICENSE` / `NOTICE` / NDX identity.
+- **Tenancy:** Single-tenant; one core instance per host/LXC is the isolation model.
+- **Source-first:** git + cargo only this phase. No prebuilt release pipeline.
+- **Roles, not monolith:** Implement **core** only. No `ui-wasm` installer.
+- **Terraform-friendly:** env/file-oriented config; idempotent where practical;
+  **no required interactive prompts**.
+- **Track installer in this repo:** `install.sh` (and helper) live here;
+  rusterp.biz may mirror later — not a separate packaging monorepo.
+- **Platforms:** Linux + macOS for bootstrap scripts.
+- **Server surface:** Do not change Parties/domain logic, auth, TLS, or multi-tenant
+  wiring. Default listen remains aligned with `rusterp-server`
+  (`127.0.0.1:50051` / `RUSTERP_LISTEN`).
+- **Toolchains:** Fail honestly if `git` or `cargo` missing; **do not** auto-install
+  rustup/cargo.
+- **Honesty:** Prefer a working core source-install path over a generic package
+  framework or theatrical “full product” installer.
+- **Build:** If any Rust is added, root `cargo check` / `cargo test` must stay green
+  with no external services. Shell-only is acceptable for Phase 3.
 
 ## Acceptance Criteria
 
-- [ ] **`PartyService`** is defined under `proto/rusterp/party/v1/` (existing
-      Party / PartyRole / Contact messages retained or extended only as needed)
-      with at least these RPCs: `CreateParty`, `GetParty`, `ListParties`,
-      `UpdateParty`, `AddContact`, `ListContacts` (request/response messages as
-      needed).
-- [ ] A minimal **Health** (or readiness) surface exists: either
-      `proto/rusterp/platform/v1/` Health service **or** an equivalent tiny ping
-      RPC — enough to prove the process is serving gRPC.
-- [ ] **Codegen pipeline** builds generated Rust from the protos via tonic/prost
-      (build.rs and/or a small dedicated codegen crate). Document how to
-      regenerate in README (or crate-level docs).
-- [ ] Workspace member **`crates/rusterp-server`** exists (binary) and is listed
-      in root `Cargo.toml` members. It runs a thin gRPC server that wires
-      **in-memory** Parties state to the generated `PartyService` impl.
-- [ ] Default listen address is **`127.0.0.1:50051`**, overridable (CLI flag
-      and/or env var — document the chosen mechanism).
-- [ ] Server serves at least **CreateParty**, **GetParty**, and **ListParties**
-      correctly against the in-memory repo (remaining PartyService RPCs
-      implemented or clearly stubbed with honest errors — prefer full minimal
-      impl of all six listed RPCs if small).
-- [ ] At least **one automated test** covers service wiring (prefer tonic
-      in-process / router test or library-level service test that does not
-      require a free TCP port; if a bind-based test is used, it must be reliable
-      in CI-like local runs).
-- [ ] **No** new functional domain crates; **no** real DB driver dependencies
-      added for application use.
-- [ ] Root **README** (and `AGENTS.md` repo map if present) documents: build,
-      run server, example `grpcurl` (or equivalent) calls for Health + at least
-      one Parties RPC, and explicit notes that **persistence is in-memory** and
-      **auth is not enforced**.
-- [ ] **`cargo check`** exits 0 (attested).
-- [ ] **`cargo test`** exits 0 (attested), including the new wiring test(s).
-- [ ] Optional: gRPC **server reflection** enabled in the binary to ease
-      `grpcurl` discovery (Decision Log default: **yes** if low-cost).
+- [ ] **`install.sh`** exists at **repo root**, documented for **Linux and macOS**.
+      It bootstraps the `rusterp` helper into a conventional path (user-local by
+      default for non-root; document any system path). Re-run is **safe/idempotent**
+      enough for operator retry. It installs **only** the CLI/helper — not the whole
+      ERP — and points operators at `rusterp install core`.
+- [ ] **`rusterp` CLI** (shell OK) supports at least:
+      - `rusterp install core [--branch <name>]`
+      - `rusterp status` (minimal: core present?, binary path, listen hint if known)
+- [ ] **Default branch** for `install core` is **`dist`** when `--branch` is omitted.
+- [ ] **`--branch <name>`** overrides the git ref used for fetch/build.
+- [ ] **Install behavior** (documented + implemented): clone/fetch this repo at the
+      chosen ref into a documented prefix; `cargo build -p rusterp-server --release`;
+      install or link the binary + a minimal run script (and optional example unit).
+- [ ] **Missing `git` or `cargo`:** non-zero exit and a clear message with next steps
+      (no silent toolchain install).
+- [ ] **Missing / unfetchable branch:** fail closed (non-zero); do **not** silently
+      fall back to `main` unless the Spec is later amended — Phase 3 default is
+      **fail-closed** on bad/missing ref.
+- [ ] **Config/run docs:** `RUSTERP_LISTEN` (and any data-dir or prefix env vars
+      introduced) listed for Terraform; how to run core after install is clear.
+- [ ] **README** section: bootstrap (`install.sh`) → `rusterp install core` → run
+      server → optional note that UI is separate and points at the gRPC endpoint.
+      States: source install needs **Rust stable + git**; default branch **`dist`**;
+      variants via `--branch`.
+- [ ] **No** `rusterp install ui-wasm` (or other roles) implemented in this change.
+- [ ] **Automated checks** suitable for attestation without requiring a live git
+      network install when possible, e.g.:
+      - shell syntax (`bash -n` / `sh -n`) on `install.sh` and `rusterp` helper
+      - unit-style tests or pure functions for default-branch / argv parsing if
+        factored out
+      - `cargo check` and `cargo test` at repo root still exit 0 (attested) if the
+        workspace is touched or unchanged
+- [ ] Manual smoke path **documented** (not required attested): clean machine/LXC
+      `install.sh` → `rusterp install core` (or a documented `--dry-run` if added).
 
 ## Out-of-Scope
 
-- Real SQLite / PostgreSQL schemas, migrations, connection pools, Litestream.
-- TLS, mTLS, rate limiting, multi-listener production ops.
-- AuthN/AuthZ, OIDC, API keys, sessions, multi-tenant routing.
-- Catalog, Sales, Payments, Inventory (or any new functional domain).
-- WASM / Macaron / any UI crate in this repo.
-- Expanding Parties business rules (merge, GDPR, addresses book, CRM pipeline).
-- Always-on Core identity domain (Users/Groups) beyond a tiny Health proto if
-  placed under `platform.v1`.
-- Guaranteeing a live network smoke in attestation if flaky; automated
-  in-process test is the honesty bar. Manual `grpcurl` is documented, not
-  required to be attested.
+- `rusterp install ui-wasm` (UI repo / later Spec).
+- Prebuilt binaries, deb/rpm, Homebrew, container images, Litestream sidecar packs.
+- Production systemd/launchd hardening beyond a **minimal** example unit or run
+  script.
+- Auto-install of rustup/cargo/protoc on the target.
+- SaaS / multi-tenant installer paths.
+- AuthN/AuthZ, TLS, real DB drivers, domain feature work.
+- Creating or maintaining the remote **`dist`** branch contents as part of this
+  change (operators/CI may pin it; installer must use the name as default).
+- Generic multi-role package framework beyond what core needs.
 
 ## Decision Log / Open Questions
 
 | Decision / Question | Status | Notes |
 |---------------------|--------|-------|
-| Phase 1 Parties accepted | **decided** | In-memory domain crate is upstream of this phase |
-| Server crate name | **decided** | `rusterp-server` under `crates/rusterp-server` (binary) |
-| Codegen stack | **decided** | tonic + prost; protos remain source of truth under `proto/` |
-| Health surface | **decided** | Separate `rusterp.platform.v1` Health (or Check) service — not bolted onto PartyService |
-| Default listen | **decided** | `127.0.0.1:50051`; override via CLI and/or env (implementer documents exact flag/env names) |
-| gRPC reflection | **decided** | **Enable** in the server binary for Phase 2 if dependency cost is modest; document `grpcurl` list/describe |
-| Shared state | **decided** | Single in-process `InMemoryPartyRepository` (mutex/async mutex as needed) — not multi-tenant |
-| Auth | **decided** | None this phase |
-| Persistence | **decided** | In-memory only |
-| Where generated code lives | **open** | Prefer small `rusterp-proto` (or `rusterp-grpc`) lib crate consumed by server — implementer picks one clear layout and documents it |
-| Async runtime | **open** | tokio expected with tonic; confirm in impl docs |
-| Exact override flag/env names | **open** | e.g. `--listen` / `RUSTERP_LISTEN` — implementer chooses and documents |
-| Full impl vs stub for Update/Add/List contacts | **decided** | Implement all six PartyService RPCs minimally (map domain errors to tonic `Status`) |
+| Phase scope | **decided** | Bootstrap + `install core` only; source-first |
+| Default git ref | **decided** | `dist` when `--branch` omitted |
+| Missing branch policy | **decided** | Fail-closed; no silent `main` fallback |
+| CLI implementation | **decided** | Shell acceptable for Phase 3 |
+| Install prefix default | **decided** | **User-local** default (e.g. `~/.local/share/rusterp` + `~/.local/bin`); document optional root/`/opt/rusterp` path if supported |
+| Bootstrap path for `rusterp` | **decided** | Prefer `~/.local/bin` (or equivalent) for non-root; document |
+| systemd/launchd | **decided** | Minimal example unit **or** run script + docs only — implementer picks leanest honest option |
+| `dist` branch existence in remote | **open / ops** | Installer defaults to name `dist`; creating the branch is outside this Spec’s code AC |
+| Exact env names beyond `RUSTERP_LISTEN` | **open** | Implementer may add e.g. `RUSTERP_HOME` / prefix vars — must document in README |
+| Dry-run flag | **open** | Optional; nice for CI/docs; not required if parsing tests cover defaults |
+| protoc on target for source build | **open** | `rusterp-server` build may need `protoc`; document as prerequisite if still required — do not auto-install |
 
 ---
 
diff --git a/.nucleus/state.json b/.nucleus/state.json
index 792f11e..8a95904 100644
--- a/.nucleus/state.json
+++ b/.nucleus/state.json
@@ -2,15 +2,16 @@
   "version": 1,
   "phase": "Reviewing",
   "role": "reviewer",
-  "changeId": "chg-2026-07-27T15-00-17-641Z-bb3fef",
+  "changeId": "chg-2026-07-27T15-30-13-948Z-afc57a",
   "specPath": ".nucleus/specs/current.md",
   "attestationIds": [
-    "att-2026-07-27T15-18-02-756Z-4bdbaef6",
-    "att-2026-07-27T15-18-06-237Z-6d9beb12"
+    "att-2026-07-27T16-10-08-671Z-ffdece8e",
+    "att-2026-07-27T16-10-09-308Z-3dbdb875",
+    "att-2026-07-27T16-10-10-011Z-49674442"
   ],
   "reviewResult": null,
   "overrideReason": null,
   "notes": [],
   "createdAt": "2026-07-27T11:34:59.283Z",
-  "updatedAt": "2026-07-27T15:19:26.396Z"
+  "updatedAt": "2026-07-27T16:11:09.124Z"
 }
diff --git a/AGENTS.md b/AGENTS.md
index 3f18ab4..46f4438 100644
--- a/AGENTS.md
+++ b/AGENTS.md
@@ -56,6 +56,8 @@ Cargo.toml                 workspace root
 AGENTS.md                  this file
 README.md                  human-facing mission + decisions
 CONTRIBUTING.md            DCO + contribution norms
+install.sh                 bootstrap rusterp helper CLI only (not full ERP)
+dist/                      rusterp CLI (shell), lib, test-dist.sh, example unit
 crates/
   rusterp-storage/         storage traits + backend stubs
   rusterp-modules/         functional module registry / activation
diff --git a/README.md b/README.md
index 969e066..7eeef90 100644
--- a/README.md
+++ b/README.md
@@ -45,6 +45,8 @@ are out of MVP scope.
 
 ```text
 Cargo.toml                 workspace root
+install.sh                 bootstrap `rusterp` helper CLI only
+dist/                      rusterp CLI, lib, offline tests, example unit
 crates/
   rusterp-storage/         storage traits + SQLite / PostgreSQL stubs
   rusterp-modules/         functional module registry / activation skeleton
@@ -121,6 +123,85 @@ cargo build -p rusterp-proto
 
 See [proto/README.md](./proto/README.md).
 
+## Install core (source bootstrap)
+
+Self-host / LXC path for the **core** gRPC role only (single-tenant). **Source
+install** — needs **git**, **stable Rust (`cargo`)**, and **`protoc`**. Does **not**
+auto-install toolchains. UI (`ui-wasm`) is a separate consumer / later install
+role — not shipped here.
+
+### 1. Bootstrap the helper CLI
+
+Linux and macOS. Installs only `rusterp` under a user-local prefix by default
+(`~/.local/share/rusterp/cli` + `~/.local/bin/rusterp`). As root, defaults are
+`/opt/rusterp` and `/usr/local/bin`. Re-run is safe (idempotent copy/symlink).
+
+```bash
+# from a checkout:
+./install.sh
+
+# or remote bootstrap of the CLI only:
+curl -fsSL https://raw.githubusercontent.com/ndx-video/RustERP/main/install.sh | bash
+```
+
+Ensure `~/.local/bin` is on `PATH` if needed.
+
+### 2. Install core from source
+
+```bash
+rusterp install core              # default git branch: dist
+rusterp install core --branch dist
+rusterp install core --branch my-pin
+rusterp status
+```
+
+- Default ref is **`dist`**. There is **no silent fallback to `main`** if the
+  branch is missing or unfetchable (fail-closed).
+- Clone/build prefix: **`RUSTERP_HOME`** (default `~/.local/share/rusterp`, or
+  `/opt/rusterp` when root).
+- Builds `rusterp-server` in **release** mode and installs:
+  - `$RUSTERP_HOME/bin/rusterp-server`
+  - `$RUSTERP_HOME/bin/run-core` (wrapper honoring `RUSTERP_LISTEN`)
+  - example env file and optional systemd unit template under `$RUSTERP_HOME`
+
+### 3. Run core
+
+```bash
+# default listen 127.0.0.1:50051
+rusterp-run-core
+# or:
+"$HOME/.local/share/rusterp/bin/run-core"
+RUSTERP_LISTEN=0.0.0.0:50051 "$HOME/.local/share/rusterp/bin/rusterp-server"
+```
+
+**Persistence is in-memory. Authentication is not enforced.** Point a separate
+UI or `grpcurl` at the listen address when ready.
+
+### Terraform-oriented environment
+
+| Variable | Purpose |
+|----------|---------|
+| `RUSTERP_HOME` | Install prefix (source tree, binaries, state) |
+| `RUSTERP_BIN_DIR` | Where `install.sh` places `rusterp` on `PATH` |
+| `RUSTERP_REPO_URL` | Git remote for `install core` (default GitHub origin) |
+| `RUSTERP_LISTEN` | gRPC listen (`host:port`, default `127.0.0.1:50051`) |
+| `RUSTERP_BOOTSTRAP_REF` | Ref used when `install.sh` downloads CLI files remotely (default `main`) |
+
+No interactive prompts. Prefer env or an `EnvironmentFile` (see
+`dist/rusterp-server.service.example`).
+
+### Offline checks / manual smoke
+
+```bash
+# no network — syntax + default-branch / argv parsing
+./dist/test-dist.sh
+```
+
+Manual smoke (not required in CI): clean LXC → `./install.sh` →
+`rusterp install core` → `rusterp status` → `run-core` → Health via `grpcurl`.
+
+Design notes (non-normative): [DIST.md](./DIST.md).
+
 ## License
 
 **RustERP by NDX Pty Ltd** is licensed under the **Apache License, Version 2.0**.

# Untracked files:
? .nucleus/attestations/att-2026-07-27T16-10-08-671Z-ffdece8e.json
? .nucleus/attestations/att-2026-07-27T16-10-08-671Z-ffdece8e.md
? .nucleus/attestations/att-2026-07-27T16-10-09-308Z-3dbdb875.json
? .nucleus/attestations/att-2026-07-27T16-10-09-308Z-3dbdb875.md
? .nucleus/attestations/att-2026-07-27T16-10-10-011Z-49674442.json
? .nucleus/attestations/att-2026-07-27T16-10-10-011Z-49674442.md
? .nucleus/out/0011.md
? .nucleus/out/0012.md
? .nucleus/specs/archive/chg-2026-07-27T15-00-17-641Z-bb3fef__accepted-pre-new__current.md
? .nucleus/specs/archive/chg-2026-07-27T15-00-17-641Z-bb3fef__accepted-pre-new__current.meta.json
? .nucleus/specs/archive/chg-2026-07-27T15-30-13-948Z-afc57a__new-change__current.md
? .nucleus/specs/archive/chg-2026-07-27T15-30-13-948Z-afc57a__new-change__current.meta.json
? DIST.md
? dist/rusterp
? dist/rusterp-lib.sh
? dist/rusterp-server.service.example
? dist/test-dist.sh
? install.sh
```

---

## 3. Attestations (integrity-verified only)


### att-2026-07-27T16-10-08-671Z-ffdece8e

| Field | Value |
|-------|-------|
| command | `./dist/test-dist.sh` |
| exitCode | **0** |
| durationMs | 70 |
| timestamp | 2026-07-27T16:10:08.672Z |
| cwd | /home/bilbo/code/RustERP |
| integrity | HMAC verified |
| git | main@bf6993cb dirty |

**stdout**
```
# syntax
ok - bash -n install.sh
ok - bash -n dist/rusterp
ok - bash -n dist/rusterp-lib.sh
ok - bash -n dist/test-dist.sh
# defaults
ok - default core branch
# parse install core args
ok - omit --branch → dist
ok - --branch value
ok - --branch= value
ok - missing --branch value
ok - unknown flag
# require_cmd
ok - require_cmd finds bash
ok - require_cmd missing tool
# ui-wasm rejected by CLI help path (grep)
ok - rusterp mentions core only in help
# install.sh is CLI-only (does not cargo build)
ok - install.sh does not invoke cargo build

1..14
# passed=14 failed=0

```

**stderr**
```
rusterp: error: required command not found: definitely-not-a-real-cmd-xyz
rusterp: install it

```

### att-2026-07-27T16-10-09-308Z-3dbdb875

| Field | Value |
|-------|-------|
| command | `cargo check` |
| exitCode | **0** |
| durationMs | 707 |
| timestamp | 2026-07-27T16:10:09.308Z |
| cwd | /home/bilbo/code/RustERP |
| integrity | HMAC verified |
| git | main@bf6993cb dirty |

**stdout**
```
(empty)
```

**stderr**
```
    Blocking waiting for file lock on package cache
    Blocking waiting for file lock on package cache
    Blocking waiting for file lock on package cache
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.63s

```

### att-2026-07-27T16-10-10-011Z-49674442

| Field | Value |
|-------|-------|
| command | `cargo test` |
| exitCode | **0** |
| durationMs | 1392 |
| timestamp | 2026-07-27T16:10:10.012Z |
| cwd | /home/bilbo/code/RustERP |
| integrity | HMAC verified |
| git | main@bf6993cb dirty |

**stdout**
```

running 1 test
test tests::register_enable_disable_inventory_style_toggle ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 5 tests
test tests::add_and_list_contacts_for_party ... ok
test tests::create_party_and_fetch_by_id ... ok
test tests::party_can_be_customer_and_supplier ... ok
test tests::unknown_party_id_returns_not_found ... ok
test tests::register_and_enable_parties_module ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 7 tests
test tests::parse_listen_flag ... ok
test tests::health_check_returns_ok ... ok
test tests::party_service_create_get_list_wiring ... ok
test tests::party_service_unknown_id_is_not_found ... ok
test tests::party_service_update_and_contacts ... ok
test tests::resolve_listen_default ... ok
test tests::build_router_succeeds ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 2 tests
test tests::postgres_stub_reports_backend_and_pings ... ok
test tests::sqlite_stub_reports_backend_and_pings ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


```

**stderr**
```
    Blocking waiting for file lock on package cache
    Blocking waiting for file lock on package cache
    Blocking waiting for file lock on build directory
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.68s
     Running unittests src/lib.rs (target/debug/deps/rusterp_modules-4343f1761d226b34)
     Running unittests src/lib.rs (target/debug/deps/rusterp_parties-54b4e292ca7bd18b)
     Running unittests src/lib.rs (target/debug/deps/rusterp_proto-22718c02856d04a9)
     Running unittests src/lib.rs (target/debug/deps/rusterp_server-f14f7c81f51c5e1b)
     Running unittests src/main.rs (target/debug/deps/rusterp_server-54d578114ce969cf)
     Running unittests src/lib.rs (target/debug/deps/rusterp_storage-06c8786b83a6fb31)
   Doc-tests rusterp_modules
   Doc-tests rusterp_parties
   Doc-tests rusterp_proto
   Doc-tests rusterp_server
   Doc-tests rusterp_storage

```

## 4. Independent re-execution

Harness re-ran each attested command. **exit_mismatch → strong FAIL.**
Optional second pass: tool `nucleus_verify`.

### Summary

| Attestation | Verdict | Exit | Stdout | Stderr |
|-------------|---------|------|--------|--------|
| `att-2026-07-27T16-10-08-671Z…` | **MATCH** | 0→0 | ok | ok |
| `att-2026-07-27T16-10-09-308Z…` | **OUTPUT MISMATCH** | 0→0 | ok | DIFF |
| `att-2026-07-27T16-10-10-011Z…` | **OUTPUT MISMATCH** | 0→0 | DIFF | DIFF |

> ⚠ **MISMATCH present** — default to FAIL unless you have a non-fabrication explanation.

### Details

### Independent re-execution: att-2026-07-27T16-10-08-671Z-ffdece8e — **MATCH**

- **command:** `./dist/test-dist.sh`
- **cwd:** /home/bilbo/code/RustERP
- **verifiedAt:** 2026-07-27T16:11:09.248Z
- **exitCode:** attested 0 vs re-exec 0 → MATCH
- **stdout:** MATCH
- **stderr:** MATCH
- **reexec durationMs:** 67

**Notes:**
- Independent re-execution matches attested exit code and output.

Re-exec stdout:
```
# syntax
ok - bash -n install.sh
ok - bash -n dist/rusterp
ok - bash -n dist/rusterp-lib.sh
ok - bash -n dist/test-dist.sh
# defaults
ok - default core branch
# parse install core args
ok - omit --branch → dist
ok - --branch value
ok - --branch= value
ok - missing --branch value
ok - unknown flag
# require_cmd
ok - require_cmd finds bash
ok - require_cmd missing tool
# ui-wasm rejected by CLI help path (grep)
ok - rusterp mentions core only in help
# install.sh is CLI-only (does not cargo build)
ok - install.sh does not invoke cargo build

1..14
# passed=14 failed=0

```
Re-exec stderr:
```
rusterp: error: required command not found: definitely-not-a-real-cmd-xyz
rusterp: install it

```

### Independent re-execution: att-2026-07-27T16-10-09-308Z-3dbdb875 — **OUTPUT MISMATCH**

- **command:** `cargo check`
- **cwd:** /home/bilbo/code/RustERP
- **verifiedAt:** 2026-07-27T16:11:09.612Z
- **exitCode:** attested 0 vs re-exec 0 → MATCH
- **stdout:** MATCH
- **stderr:** DIFFERS
- **reexec durationMs:** 362

**Notes:**
- stderr differs from attestation (after normalize).
- Exit codes match but output differs — treat as suspicious (flaky tests, non-determinism, or drift).

Re-exec stdout:
```
(empty)
```
Re-exec stderr:
```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

```

### Independent re-execution: att-2026-07-27T16-10-10-011Z-49674442 — **OUTPUT MISMATCH**

- **command:** `cargo test`
- **cwd:** /home/bilbo/code/RustERP
- **verifiedAt:** 2026-07-27T16:11:10.396Z
- **exitCode:** attested 0 vs re-exec 0 → MATCH
- **stdout:** DIFFERS
- **stderr:** DIFFERS
- **reexec durationMs:** 784

**Notes:**
- stdout differs from attestation (after normalize).
- stderr differs from attestation (after normalize).
- Exit codes match but output differs — treat as suspicious (flaky tests, non-determinism, or drift).

Re-exec stdout:
```

running 1 test
test tests::register_enable_disable_inventory_style_toggle ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 5 tests
test tests::add_and_list_contacts_for_party ... ok
test tests::create_party_and_fetch_by_id ... ok
test tests::party_can_be_customer_and_supplier ... ok
test tests::unknown_party_id_returns_not_found ... ok
test tests::register_and_enable_parties_module ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 7 tests
test tests::parse_listen_flag ... ok
test tests::health_check_returns_ok ... ok
test tests::party_service_unknown_id_is_not_found ... ok
test tests::party_service_update_and_contacts ... ok
test tests::party_service_create_get_list_wiring ... ok
test tests::resolve_listen_default ... ok
test tests::build_router_succeeds ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 2 tests
test tests::postgres_stub_reports_backend_and_pings ... ok
test tests::sqlite_stub_reports_backend_and_pings ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


```
Re-exec stderr:
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.19s
     Running unittests src/lib.rs (target/debug/deps/rusterp_modules-4343f1761d226b34)
     Running unittests src/lib.rs (target/debug/deps/rusterp_parties-54b4e292ca7bd18b)
     Running unittests src/lib.rs (target/debug/deps/rusterp_proto-22718c02856d04a9)
     Running unittests src/lib.rs (target/debug/deps/rusterp_server-f14f7c81f51c5e1b)
     Running unittests src/main.rs (target/debug/deps/rusterp_server-54d578114ce969cf)
     Running unittests src/lib.rs (target/debug/deps/rusterp_storage-06c8786b83a6fb31)
   Doc-tests rusterp_modules
   Doc-tests rusterp_parties
   Doc-tests rusterp_proto
   Doc-tests rusterp_server
   Doc-tests rusterp_storage

```

---

## Required output

1. **Verdict:** PASS or FAIL
2. **Findings:** bullets (fabrication, missing evidence, drift, Spec, re-exec)
3. **Evidence notes:** which att + re-exec fields you used
4. **Next step:** Accept / Reject / Re-implement / Re-attest

**Rules:** exit_mismatch → strong FAIL · output_mismatch → suspicious · Spec drift → FAIL

--- END REVIEW BUNDLE ---

Review now. Be skeptical. Optional second pass: `nucleus_verify`.
Human records outcome: `/review pass` or `/review fail <summary>`.