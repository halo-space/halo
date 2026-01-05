//! 异步版本：A 调 B，B 调 C，C 调 D；外层 WithTimeout(5s)，C 内部 10s 睡眠被超时抢占。
#![allow(non_snake_case)]

use std::time::Duration;

use core::context::{AfterFunc, Background, Context, ContextAware, ContextError, WithTimeout};
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    // 整体超时 5s。
    let (ctx, cancel) = WithTimeout(Background(), Duration::from_secs(5));

    // 注册 AfterFunc 观测取消。
    let done_flag = AfterFunc(&ctx, || {
        println!("[AfterFunc] context canceled (deadline exceeded)");
    });
    drop(done_flag);

    // 直接异步调用 A（无额外线程）。
    match ContextAware(ctx.clone(), a(ctx)).await {
        Ok(_) => println!("[main] A completed"),
        Err(e) => println!("[main] A failed: {e}"),
    }

    // 即便超时，依然显式 cancel，保持 Go 语义。
    cancel();
}

async fn a(ctx: Context) -> Result<(), ContextError> {
    println!("[A] start");
    b(&ctx).await?;
    println!("[A] end");
    Ok(())
}

async fn b(_ctx: &Context) -> Result<(), ContextError> {
    println!("[B] start");
    c(_ctx).await?;
    println!("[B] end");
    Ok(())
}

async fn c(_ctx: &Context) -> Result<(), ContextError> {
    println!("[C] start: simulate 10s work (opaque, no cancel checks)");
    sleep(Duration::from_secs(10)).await;
    d(_ctx).await?;
    println!("[C] end");
    Ok(())
}

async fn d(ctx: &Context) -> Result<(), ContextError> {
    println!("[D] start");
    if let Some(err) = ctx.err() {
        println!("[D] canceled: {err}");
        return Err(err);
    }
    println!("[D] end");
    Ok(())
}
