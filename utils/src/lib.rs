//! `utils`：通用工具库（路径、模板定位、模板渲染、模板安装）。
//!
//! 说明：
//! - 该 crate 设计为与 `core/rest/rsctl` 并列的通用工具包；
//! - 其中的渲染器是“最小 go-template 风格”，服务于 rsctl 的模板系统；
//! - 该 crate 不依赖 rsctl 的业务逻辑（adapter/生成器等都在 rsctl 内部）。

pub mod path;

/// 模板相关能力（仅 rsctl 等代码生成工具需要）。
#[cfg(feature = "template")]
pub mod render;

/// 模板相关能力（仅 rsctl 等代码生成工具需要）。
#[cfg(feature = "template")]
pub mod template;

/// JWT 纯工具（编解码/校验），不包含任何 Web 框架相关逻辑。
#[cfg(feature = "jwt")]
pub mod jwt;
