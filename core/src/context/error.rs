use std::fmt::{Display, Formatter};
use std::sync::Arc;

/// Go 语义下的上下文错误类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// 对齐 Go 的 `context canceled`。
    Canceled,
    /// 对齐 Go 的 `context deadline exceeded`。
    DeadlineExceeded,
}

impl Error {
    pub const fn as_str(self) -> &'static str {
        match self {
            Error::Canceled => "context canceled",
            Error::DeadlineExceeded => "context deadline exceeded",
        }
    }
}

/// Context 错误，带可选 cause。
#[derive(Debug, Clone)]
pub struct ContextError {
    kind: Error,
    cause: Option<Arc<dyn std::error::Error + Send + Sync>>,
}

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
