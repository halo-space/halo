//! `halo`：对外统一入口（facade crate）。
//!
//! 推荐使用方式：
//! - `halo::core::...`
//! - `halo::rest::...`
//!
//! # crates.io package 名
//! 本项目发布到 crates.io 的 package 名为 `halo-micro`，但库 crate 名固定为 `halo`。
//! 因此使用方 Cargo.toml 推荐这样写：
//!
//! ```toml
//! halo = { package = "halo-micro", version = "0.1.0" }
//! ```
//!
//! 注意：
//! - 对外统一通过 `halo::core` / `halo::rest` 访问。

/// `halo::core::*` -> 本项目 `core` crate。
pub mod core {
    pub use ::core::*;
}

/// 兼容 go-zero 风格：`halo::conf::must_load(...)`。
pub mod conf {
    pub use ::core::conf::*;
}

/// `halo::rest::*` -> `rest` crate。
pub mod rest {
    pub use ::rest::*;
}

#[cfg(test)]
mod tests {
    #[test]
    fn facade_types_should_point_to_workspace_core() {
        // 如果这里能赋值成功，就证明 `halo::conf` 导出的是 workspace 的 `core::conf`，
        // 而不是标准库的 `core`。
        let _: ::core::conf::Format = crate::conf::Format::Yaml;
    }
}
