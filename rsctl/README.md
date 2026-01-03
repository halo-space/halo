# rsctl

> Rust codegen inspired by go-zero templates.
> 中文说明：见 [README.zh.md](README.zh.md)

## Install

```bash
cargo install --git https://github.com/halo-space/halo.git --bin rsctl --force
```

## Template management

Templates are stored under `~/.rsctl/<VERSION>/`:

```bash
rsctl template init    # install current version templates (no overwrite)
rsctl template update  # force reinstall current version templates
rsctl template clean   # remove current version templates
```

## CLI (goctl-style)

Top-level commands now follow `api rs` / `api openapi`:

### Generate Rust service code

```bash
rsctl api rs \
  --api rsctl/test/api.api \
  --dir rsctl/test/out \
  --merge true \
  --fmt true \
  --overwrite
```

Key flags:
- `--api`: path to `.api` file (supports `import`).
- `--dir`: output directory.
- `--merge`: merge handlers of the same group (default true).
- `--style`: file naming style `rust_zero` | `rustZero` | `RustZero`.
- `--remote`: template source (local path or git/http).
- `--fmt`: run `cargo fmt` after generation (default true).
- `--overwrite`: overwrite existing files.

Routing/root prefix & group:
- `info.root_prefix` from `.api` sets the service root (default `/`).
- `@server(prefix="...")` is appended per group.
- Debug print (dev/test) shows routes grouped by `@server(group="...")`.

Struct field naming:
- JSON schema and generated Rust structs honor `json` tags (e.g. `json:"userId"` -> `#[serde(rename = "userId")] pub user_id: ...`).

### Generate OpenAPI v3

```bash
rsctl api openapi \
  --api rsctl/test/api.api \
  --dir rsctl/test/api.json \
  --filename openapi \
  --format yaml \  # or json (default)
  --overwrite
```

Behavior:
- Top-level order: `openapi` → `info` → `paths` → `components`.
- `info` fields from `.api` `info` block (`title/desc/version/root_prefix`).
- `requestBody` included; schemas built from `type` definitions and `json` tags.
- `components.schemas` emitted; `$ref` used when possible.

## Run generated service (example)

```bash
cd rsctl/test/out
cargo run
```

Edit `etc/<service>.yaml` to change host/port/mode.

## License

Apache-2.0 (see repository root `LICENSE`).

## Acknowledgements

Thanks to the go-zero project for the inspiring API/templating model.