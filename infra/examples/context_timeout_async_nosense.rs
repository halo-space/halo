//! 异步版（无感知）：A 调 B，B 调 C，C 调 D；业务不检查取消，用 ContextAware 抢占。
#![allow(non_snake_case)]

use std::time::Duration;

use infra::context::{AfterFunc, Background, Context, ContextAware, ContextError, WithTimeout};
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let (ctx, cancel) = WithTimeout(Background(), Duration::from_secs(5));
    let done_flag = AfterFunc(&ctx, || {
        println!("[AfterFunc] context canceled (deadline exceeded)");
    });
    drop(done_flag);

    match ContextAware(ctx.clone(), a(ctx.clone())).await {
        Ok(_) => println!("[main] A completed"),
        Err(e) => println!("[main] A failed: {e}"),
    }

    cancel();
}

async fn a(ctx: Context) -> Result<(), ContextError> {
    b(&ctx).await
}

async fn b(ctx: &Context) -> Result<(), ContextError> {
    c(ctx).await?;
    d(ctx).await?;
    Ok(())
}

async fn c(_ctx: &Context) -> Result<(), ContextError> {
    // 无感知，单纯耗时 10s。
    sleep(Duration::from_secs(10)).await;
    d(_ctx).await
}

async fn d(ctx: &Context) -> Result<(), ContextError> {
    if let Some(err) = ctx.err() {
        return Err(err);
    }
    Ok(())
}
