# Protobuf / gRPC layout

RustERP is **API-first and headless**. External clients (including the Macaron-style
egui WASM UI in [RustERP-UI-WASM](https://github.com/ndx-video/RustERP-UI-WASM))
consume **gRPC** services defined here as Protocol Buffers.

## Transport

| Consumer | Transport | Endpoint |
|----------|-----------|----------|
| API tools (`grpcurl`, integrations) | Native TCP gRPC + HTTP/2 | `127.0.0.1:50051` (`RUSTERP_LISTEN`) |
| Browser / WASM UI | **slozhn** gRPC-over-WebSocket | `ws://host:8123/rpc` (`RUSTERP_HTTP_LISTEN`) |

There is **no JSON/REST** domain API — wire shape is Protobuf only.

## Conventions

| Item | Convention |
|------|------------|
| Package naming | `rusterp.<area>.v1` (e.g. `rusterp.party.v1`) |
| File layout | One area per directory under `proto/rusterp/` |
| Compatibility | Prefer additive evolution within a `v1` package; breaking changes → `v2` |
| Codegen | `crates/rusterp-proto` via `tonic-prost-build` + `prost` (requires `protoc`) |

## Tree

```text
proto/
  README.md                 ← this file
  rusterp/
    platform/v1/            ← Health (minimal); identity/RBAC later
    party/v1/               ← Parties messages + PartyService
    catalog/v1/             ← future
    sales/v1/
    ...
```

## Codegen / regenerate

Protobuf sources in this tree are compiled by `crates/rusterp-proto/build.rs`.

```bash
# Requires protoc on PATH
cargo build -p rusterp-proto
# or rebuild consumers
cargo build -p rusterp-server
```

Generated Rust is emitted into Cargo `OUT_DIR` and included from
`rusterp-proto` (`rusterp_proto::party::v1`, `rusterp_proto::platform::v1`).
Do not hand-edit generated output.

The gRPC server binary is `rusterp-server` (in-memory Parties store; **auth not
enforced**). See the root README for run, dual-port, and `grpcurl` examples.
