//! `halo_micro` facade.
//!
//! 推荐使用方式：
//! - `halo_micro::infra::...`
//! - `halo_micro::rest::...`
//!
//! # crates.io package 名
//! 发布包名：`halo-micro`，库 crate 名：`halo_micro`。
//! 使用示例：
//! ```toml
//! halo_micro = { package = "halo-micro", version = "0.1.0" }
//! ```
//!
//! 注意：
//! - 对外统一通过 `halo_micro::infra` / `halo_micro::rest` 访问。

/// `halo_micro::infra::*` -> 本项目 `infra` crate。
pub mod infra {
    pub use infra::*;
}

/// 兼容 go-zero 风格：`halo_micro::conf::must_load(...)`。
pub mod conf {
    pub use infra::conf::*;
}

/// `halo_micro::rest::*` -> `rest` crate。
pub mod rest {
    pub use ::rest::*;
}

#[cfg(test)]
mod tests {
    #[test]
    fn facade_types_should_point_to_workspace_infra() {
        // 如果这里能赋值成功，就证明 `halo_micro::conf` 导出的是 workspace 的 `infra::conf`，
        let _: infra::conf::Format = crate::conf::Format::Yaml;
    }
}
