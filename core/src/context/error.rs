use std::fmt::{Display, Formatter};
use std::sync::Arc;

/// Go 语义下的上下文错误类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// 对齐 Go 的 `context canceled`。
    Canceled,
    /// 对齐 Go 的 `context deadline exceeded`。
    DeadlineExceeded,
    /// 自定义错误消息（不与取消/超时混淆）。
    Any,
}

impl Error {
    pub const fn as_str(self) -> &'static str {
        match self {
            Error::Canceled => "context canceled",
            Error::DeadlineExceeded => "context deadline exceeded",
            Error::Any => "context error",
        }
    }
}

/// Context 错误，带可选 cause。
#[derive(Debug, Clone)]
pub struct ContextError {
    kind: Error,
    cause: Option<Arc<dyn std::error::Error + Send + Sync>>,
}

#[derive(Debug, Clone, Copy)]
struct StaticStrError(&'static str);

impl Display for StaticStrError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for StaticStrError {}

impl ContextError {
    pub const fn new(kind: Error) -> Self {
        Self { kind, cause: None }
    }

    pub fn with_cause(
        kind: Error,
        cause: Option<Arc<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self { kind, cause }
    }

    /// 简写：Any + 静态字符串。
    pub fn new_message(msg: &'static str) -> Self {
        Self::with_cause(Error::Any, Some(Arc::new(StaticStrError(msg))))
    }

    pub const fn kind(&self) -> Error {
        self.kind
    }

    pub fn cause(&self) -> Option<&Arc<dyn std::error::Error + Send + Sync>> {
        self.cause.as_ref()
    }
}

impl Display for ContextError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(cause) = &self.cause {
            write!(f, "{}: {cause}", self.kind.as_str())
        } else {
            f.write_str(self.kind.as_str())
        }
    }
}

impl std::error::Error for ContextError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause
            .as_ref()
            .map(|c| &**c as &(dyn std::error::Error + 'static))
    }
}

/// Go 对齐的默认错误实例。
pub const CANCELLED: ContextError = ContextError::new(Error::Canceled);
pub const DEADLINE_EXCEEDED: ContextError = ContextError::new(Error::DeadlineExceeded);

impl From<Error> for ContextError {
    fn from(kind: Error) -> Self {
        ContextError::new(kind)
    }
}
