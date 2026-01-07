//! 链式异步调用：A -> B -> C -> D -> F，B 内睡 5s，D 内睡 3s。
//! 外层 WithTimeout 4s，通过 ContextAware 抢占超时。
#![allow(non_snake_case)]

use std::time::Duration;

use core::context::{Background, Context, ContextAware, ContextError, WithTimeout};
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let (ctx, cancel) = WithTimeout(Background(), Duration::from_secs(4));

    let res = ContextAware(ctx.clone(), a(ctx.clone())).await;
    cancel();

    match res {
        Ok(_) => println!("[main] A completed"),
        Err(e) => println!("[main] A failed: {e}"),
    }
}

async fn a(ctx: Context) -> Result<(), ContextError> {
    println!("[A] start");
    b(&ctx).await?;
    println!("[A] end");
    Ok(())
}

async fn b(ctx: &Context) -> Result<(), ContextError> {
    println!("[B] start: sleep 5s");
    sleep(Duration::from_secs(5)).await;
    c(ctx).await?;
    println!("[B] end");
    Ok(())
}

async fn c(ctx: &Context) -> Result<(), ContextError> {
    println!("[C] start");
    d(ctx).await?;
    println!("[C] end");
    Ok(())
}

async fn d(ctx: &Context) -> Result<(), ContextError> {
    println!("[D] start: sleep 3s");
    sleep(Duration::from_secs(3)).await;
    f(ctx).await?;
    println!("[D] end");
    Ok(())
}

async fn f(ctx: &Context) -> Result<(), ContextError> {
    if let Some(err) = ctx.err() {
        println!("[F] canceled: {err}");
        return Err(err);
    }
    println!("[F] ok");
    Ok(())
}
