use hyper::Body;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Unified boxed response future type.
pub type BoxResponseFuture = Pin<Box<dyn Future<Output = http::Response<Body>> + Send>>;

/// Primary handler type (newtype; no alias to avoid ambiguity).
#[derive(Clone)]
pub struct HandlerFunc(Arc<dyn Fn(http::Request<Body>) -> BoxResponseFuture + Send + Sync>);

impl HandlerFunc {
    /// Create handler from closure.
    pub fn new<F, Fut>(f: F) -> Self
    where
        F: Fn(http::Request<Body>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = http::Response<Body>> + Send + 'static,
    {
        let f = Arc::new(f);
        Self(Arc::new(move |req| {
            let f = f.clone();
            Box::pin(async move { (f)(req).await })
        }))
    }

    /// Invoke handler.
    pub fn call(&self, req: http::Request<Body>) -> BoxResponseFuture {
        (self.0)(req)
    }
}

impl<F, Fut> From<F> for HandlerFunc
where
    F: Fn(http::Request<Body>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = http::Response<Body>> + Send + 'static,
{
    fn from(f: F) -> Self {
        HandlerFunc::new(f)
    }
}
