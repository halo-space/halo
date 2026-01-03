# 变更日志（CHANGELOG）

本项目遵循语义化版本（Semantic Versioning）。

## [未发布]

### 新增
- **语言一级命令结构**：新增 `rsctl rs ...` / `rsctl go ...` 命令入口，为未来扩展 Go 生成器预留稳定的分发层。
- **API pipeline 按语言拆分**：`rsctl/src/pipeline/api/{rs,go}.rs`，其中 Go 为占位实现（返回“未实现”错误），Rust 为完整实现。

### 变更
- **CLI 结构调整（破坏性）**：原 `rsctl api rs ...` 调整为 `rsctl rs api ...`（后续如需兼容旧命令再单独评估）。
- **模板变量统一**：模板侧统一使用 `imports` 作为导入片段变量，移除重复/历史变量（如 `importPackages` / `ImportPackages` 等）。
- **Auth 约定收敛**：API 注解仅支持 `@server(auth: Xxx)` 作为认证入口，不再支持旧的 `jwt:` 约定。

### 修复
- **生成工程依赖路径修复**：当输出目录在仓库外（例如 `/tmp`）时，生成工程 `Cargo.toml` 里对 `halo-micro` 的 `path` 指向正确的 crate 目录，避免指到 workspace 虚拟清单导致无法编译。

### 工程化/重构
- **语义层职责上移**：将类型映射、tag 解析、validate tag 转换、HTTP 路径拼接等“规则类逻辑”上移到 `semantic/api/rs.rs`，code 层只负责组装渲染所需数据并输出文件。
- **函数命名优化**：语义层函数命名改为见名知意，去除 `api_*` 前缀（例如 `ty_to_rust`、`parse_go_tag_kv`、`validate_tag_to_attrs`、`join_http_paths`）。

### 破坏性变更
- **命令行接口调整**：推荐使用 `rsctl rs api ...`，旧命令结构不再保留。


