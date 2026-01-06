//! 线程版（有感知）：A 调 B，B 调 C，C 调 D；C 内部用轮询模拟 select，外层 WithTimeout(5s)。
#![allow(non_snake_case)]

use std::thread;
use std::time::Duration;

use halo_micro::core::context::{AfterFunc, Background, Context, ContextError, WithTimeout};

fn main() {
    // 整体超时 5s。
    let (ctx, cancel) = WithTimeout(Background(), Duration::from_secs(5));

    // 观测取消。
    let done_flag = AfterFunc(&ctx, || {
        println!("[AfterFunc] context canceled (deadline exceeded)");
    });
    drop(done_flag);

    // 在独立线程调用 A。
    let handle = thread::spawn(move || match a(ctx) {
        Ok(_) => println!("[main] A completed"),
        Err(e) => println!("[main] A failed: {e}"),
    });

    handle.join().expect("thread join");
    cancel();
}

fn a(ctx: Context) -> Result<(), ContextError> {
    println!("[A] start");
    b(ctx)?;
    println!("[A] end");
    Ok(())
}

fn b(ctx: Context) -> Result<(), ContextError> {
    println!("[B] start");
    c(&ctx)?;
    d(&ctx)?;
    println!("[B] end");
    Ok(())
}

fn c(ctx: &Context) -> Result<(), ContextError> {
    println!("[C] start: simulate 10s work (poll cancel every 100ms)");
    let mut elapsed = 0u64;
    while elapsed < 10_000 {
        if let Some(err) = ctx.err() {
            println!("[C] canceled at {elapsed}ms: {err}");
            return Err(err);
        }
        thread::sleep(Duration::from_millis(100));
        elapsed += 100;
    }
    d(ctx)?;
    println!("[C] end");
    Ok(())
}

fn d(ctx: &Context) -> Result<(), ContextError> {
    if let Some(err) = ctx.err() {
        println!("[D] canceled: {err}");
        return Err(err);
    }
    Ok(())
}
