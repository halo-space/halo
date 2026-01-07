# halo-micro

参考go-zero：包含基础库、Web 框架适配层，以及代码生成工具 `rsctl`。

## 技术栈

- Rust edition 2024
- Cargo（依赖管理与构建）
- 代码风格：`rustfmt` + `clippy`
- CI：GitHub Actions（规划/持续完善）

## 项目结构

```text
halo/
  core/        # 基础核心库
  rest/        # Web 适配层/中间件/DSL
  halo/        # 门面 crate（package=halo-micro，lib crate 名为 `halo_micro`）
  utils/       # 通用工具 crate（halo-utils）
  rsctl/       # 代码生成器与模板
  LICENSE
  README.md
```

## 使用说明

- **rsctl（代码生成器）**：请看 `rsctl/README.md`
- **rest（Web 适配层）**：请看 `rest/` 下源码与示例（后续会补充文档）

## 发布到 crates.io（标准流程）

本仓库是 workspace，发布顺序固定为：

- `halo-infra`（原 `halo-core`）
- `halo-rest`
- `halo-micro`（**package 名**；lib crate 名为 `halo_micro`）

### halo crate 最后的考虑
```
 因为rust的crate很蹩脚，一个crate被占用了，其他人就不能用了，所以就尽可能采用较少的crate形式来组织。希望官方可以持续改进，比如一个组织下面,可以发布很多crate，引用的时候从这个组织下面开始，在到具体的crate上,不同组织下面的crate可以重复。这样会友好很好。当然这只是自己的想法，勿喷！！！
```

使用方依赖写法示例：

```toml
halo_micro = { package = "halo-micro", version = "0.1.0" }
```


## 许可证

本项目采用 Apache-2.0，详见 `LICENSE`。

## 版本记录

详见 `CHANGELOG.md`。
