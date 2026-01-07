//! 异步版（有感知）：A 调 B，B 调 C，C 调 D；C 用 tokio::select! 等待 ctx.done_async 与 10s 工作。
#![allow(non_snake_case)]

use std::time::Duration;

use core::context::{AfterFunc, Background, Context, ContextError, Error, WithTimeout};
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let (ctx, cancel) = WithTimeout(Background(), Duration::from_secs(5));
    let done_flag = AfterFunc(&ctx, || {
        println!("[AfterFunc] context canceled (deadline exceeded)");
    });
    drop(done_flag);

    match a(ctx).await {
        Ok(_) => println!("[main] A completed"),
        Err(e) => println!("[main] A failed: {e}"),
    }

    cancel();
}

async fn a(ctx: Context) -> Result<(), ContextError> {
    println!("[A] start");
    b(&ctx).await?;
    println!("[A] end");
    Ok(())
}

async fn b(ctx: &Context) -> Result<(), ContextError> {
    println!("[B] start");
    c(ctx).await?;
    println!("[B] end");
    Ok(())
}

async fn c(ctx: &Context) -> Result<(), ContextError> {
    println!("[C] start: 10s work with select on ctx");
    tokio::select! {
        _ = sleep(Duration::from_secs(10)) => {
            d(ctx).await?;
            println!("[C] end");
            Ok(())
        }
        _ = ctx.done_async() => {
            let err = ctx.err().unwrap_or_else(|| {
                ContextError::new(Error::Canceled)
            });
            println!("[C] canceled: {err}");
            Err(err)
        }
    }
}

async fn d(ctx: &Context) -> Result<(), ContextError> {
    if let Some(err) = ctx.err() {
        println!("[D] canceled: {err}");
        return Err(err);
    }
    Ok(())
}
