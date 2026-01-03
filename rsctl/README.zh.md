# rsctl

> Rust 代码生成器，风格借鉴 go-zero 模板。
> English: see [README.md](README.md)

## 安装

```bash
cargo install --git https://github.com/halo-space/halo.git --bin rsctl --force
```

## 模板管理

模板存放于 `~/.rsctl/<VERSION>/`：

```bash
rsctl template init    # 安装当前版本模板（不覆盖）
rsctl template update  # 覆盖安装当前版本模板
rsctl template clean   # 清理当前版本模板
```

## 命令（goctl 风格）

顶层命令：`api rs` / `api openapi`。

### 生成 Rust 服务代码

```bash
rsctl api rs \
  --api rsctl/test/api.api \
  --dir rsctl/test/out \
  --merge true \
  --fmt true \
  --overwrite
```

主要参数：
- `--api`：`.api` 文件路径（支持 `import`）。
- `--dir`：输出目录。
- `--merge`：同组 handler 是否合并（默认 true）。
- `--style`：文件命名风格 `rust_zero` | `rustZero` | `RustZero`。
- `--remote`：模板来源（本地路径或 git/http）。
- `--fmt`：生成后执行 `cargo fmt`（默认 true）。
- `--overwrite`：覆盖已存在文件。

路由与分组：
- `.api` 中 `info.root_prefix` 作为服务根前缀（默认 `/`）。
- `@server(prefix="...")` 为分组前缀；调试输出按 `@server(group="...")` 分组。

结构体字段命名：
- 遵循 `json` 标签（如 `json:"userId"` -> `#[serde(rename = "userId")] pub user_id: ...`），OpenAPI schema 同步使用。

### 生成 OpenAPI v3

```bash
rsctl api openapi \
  --api rsctl/test/api.api \
  --dir rsctl/test/api.json \
  --filename openapi \
  --format yaml \  # 或 json（默认）
  --overwrite
```

行为说明：
- 顶层顺序：`openapi` → `info` → `paths` → `components`。
- `info` 取自 `.api` 的 `info` 块（title/desc/version/root_prefix 等）。
- 包含 `requestBody`；`components.schemas` 来自 `type` 定义和 `json` 标签。
- `$ref` 优先引用已有 schema。

## 运行生成的服务（示例）

```bash
cd rsctl/test/out
cargo run
```

可编辑 `etc/<service>.yaml` 修改 host/port/mode 等配置。

## 许可证

Apache-2.0（详见仓库根目录 `LICENSE`）。

## 致谢

感谢 go-zero 项目在 API/模板设计上的启发。

