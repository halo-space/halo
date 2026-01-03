// Code scaffolded by rsctl. Safe to edit.
// rsctl {{ version }}

{{ imports }}

pub struct {{ Name }} {}

impl {{ Name }} {
    pub fn new() -> Self {
        Self {}
    }

    pub fn handle(&self) -> Middleware {
        rest::middleware::middleware(|req: Request<Body>, next: HandlerFunc| async move {
            // TODO: middleware logic
            next.call(req).await
        })
    }
}
