# RustERP distribution ideas

## Design principles (aligned with Pi + single-tenant)

| Principle | Implication |
|-----------|-------------|
| **Snug fit** | Install only what that host needs (`core`, `ui-wasm`, later `proxy`, etc.) |
| **Source of truth = git** | Prefer “pull + build” or “pull release artifact” over opaque binaries only |
| **Single-tenant** | One instance per LXC/VM; isolation is the security boundary |
| **Terraform-friendly** | Declarative inputs (version, endpoint, listen addr, data dir); idempotent apply |
| **No SaaS assumptions** | Self-host path first; NDX SaaS is a later consumer of the same packages |

---

## Option space (ranked for MVP)

### 1. **Thin CLI + git/source build (recommended first path)**

A small `rusterp` CLI (Rust binary, or a thin shell wrapper that calls it) that:

```text
rusterp install core [--version v0.1.0 | --ref main]
rusterp install ui-wasm
rusterp status
rusterp upgrade core
```

**What it does under the hood (honest):**
- Ensures Rust toolchain (or documents that it must exist)
- Clones/fetches the monorepo (or the specific crate path) into a known prefix, e.g. `/opt/rusterp` or `~/.local/share/rusterp`
- `cargo build --release -p rusterp-server` (and UI as appropriate)
- Installs a systemd/launchd unit **or** a simple supervised run script
- Writes a minimal config (`RUSTERP_LISTEN`, data dir, etc.)

**Pros:** Matches Pi’s “install the thing from source” feel; debuggable; no binary-distribution pipeline yet.  
**Cons:** Needs a compiler on the host (or a build stage in the image). Not ideal for locked-down prod boxes without a build image.

Shell-only is fine for v0 if the CLI is still evolving — a `rusterp` bash/zsh script that calls `git` + `cargo` is acceptable until the Rust CLI stabilizes. Don’t pretend it’s a full package manager.

### 2. **Release artifacts + CLI (second step)**

GitHub Releases (or your own) ship:
- `rusterp-server` binary (linux-x86_64, aarch64; maybe musl)
- `rusterp-ui` assets or a native UI binary
- Optional container image

CLI becomes:

```text
rusterp install core --from-release v0.1.0
```

**Pros:** Fast install, no toolchain on target.  
**Cons:** You maintain release CI and multi-arch builds early.

### 3. **Containers as the primary unit (Terraform’s friend)**

Treat **one image (or compose stack) per role**:

- `rusterp-core` — server only  
- `rusterp-ui` — static WASM or native UI sidecar if needed  

Terraform/OpenTofu then owns:

- LXC or VM (Proxmox provider, or cloud)
- Network, volume for PostgreSQL data
- `cloud-init` or entrypoint that sets `RUSTERP_LISTEN`, endpoint URLs

CLI still useful **inside** the image or on the host for “install this role into this prefix,” but Terraform becomes the preferred way to *place* instances.

---

## Recommended shape for RustERP

```text
rusterp (CLI)
├── install <role>     # core | ui-wasm | (later) proxy
├── uninstall <role>
├── status
├── upgrade <role>
└── config             # show/set endpoint, listen, data-dir

Roles = packages of behavior, not “the whole ERP”
```

**Roles map cleanly to Terraform modules:**

```hcl
module "rusterp_core" {
  source  = "..." # or local module
  version = "0.1.0"
  listen  = "0.0.0.0:50051"
  data_dir = "/var/lib/rusterp"
  # single-tenant: one module instance per LXC
}

module "rusterp_ui" {
  source           = "..."
  grpc_endpoint    = module.rusterp_core.endpoint
}
```

That is snug-fit: a consultant deploys **only** core + UI (or only core for headless integrations).

---

## Pull-source vs binary — practical advice

| Audience | Prefer |
|----------|--------|
| Dev / early adopters / consultants who already have Rust | **Source install** (`git` + `cargo`) via `rusterp install` |
| Prod LXC images you bake | **Build in image build**, ship binary or image; CLI only configures |
| Terraform-managed fleet | **Image + config**, not compile-on-apply |

Compile-on-target on every `terraform apply` is painful (time, flaky networks, toolchain drift). Use:

- **Dev path:** `rusterp install core` → source build  
- **Ops path:** prebuilt image or release binary; Terraform sets version + config  

Same CLI surface; different backends (`--from-source` vs `--from-release` / image tag).

---

## Pi-like UX without overbuilding

Pi’s strength is: **one clear install command, local-first, project-scoped**. Mirror that:

```text
# on a fresh LXC
curl -fsSL https://rusterp.biz/install.sh | sh   # optional bootstrap of the CLI only
rusterp install core
rusterp install ui-wasm   # optional on same or different host
```

`install.sh` should **only** install the `rusterp` CLI (or document `cargo install`), not the whole ERP. Then roles are explicit.

Avoid a mega-installer that silently pulls half the internet. Explicit roles = snug fit.

---

## Terraform-friendly checklist (bake in early)

1. **Config via env + one file** (`/etc/rusterp/core.env` or similar) — no required interactive prompts  
2. **Stable data directory** and documented layout (PostgreSQL connection URI)  
3. **Health endpoint** already exists — use it for readiness probes  
4. **Version pin** on install (`--version` / image tag)  
5. **Idempotent install** (re-run safe)  
6. **No hard-coded hostnames** — endpoint is always configurable (`RUSTERP_GRPC_ENDPOINT` already matches the UI Spec)

---

## Suggested decision for now

| Decision | Recommendation |
|----------|----------------|
| Primary install UX | `rusterp` CLI with **roles**: `core`, `ui-wasm`, … |
| First implementation | Thin shell or Rust CLI that does **source install** for core (and UI) |
| Prod / Terraform | Container (or static binary) per role; Terraform modules per role |
| Compile on target | Dev/default only; not the Terraform happy path |
| Scope for next product phase | Document this model in README/AGENTS; implement CLI **after** Parties UI proves gRPC |

You do **not** need a full package ecosystem before Catalog. A documented install story + a 100-line `install.sh` + clear env vars is enough for early self-hosters and Terraform experiments.

---

When the UI implement finishes, we can either accept that phase or open a short “install story / CLI skeleton” Spec if you want it next. Prefer finishing the live Parties list first so deploy docs can say “point UI at core on :50051” with a real smoke path.