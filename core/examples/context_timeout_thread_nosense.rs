//! 线程版（无感知）：A 调 B，B 调 C，C 调 D；业务不检查取消，外侧通过 watcher 抢占超时。
#![allow(non_snake_case)]

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use halo_core::context::{AfterFunc, Background, Context, ContextError, Error, WithTimeout};

fn main() {
    let (ctx, cancel) = WithTimeout(Background(), Duration::from_secs(5));
    let done_flag = AfterFunc(&ctx, || {
        println!("[AfterFunc] context canceled (deadline exceeded)");
    });
    drop(done_flag);

    let (tx, rx) = mpsc::channel();
    let worker_ctx: Context = ctx.clone();
    thread::spawn(move || {
        let _ = tx.send(a(worker_ctx));
    });

    // 等待结果或 ctx 完成（超时/取消）。
    let result = loop {
        if ctx.done().wait_timeout(Duration::from_millis(100)) {
            break Err(ctx
                .err()
                .unwrap_or_else(|| ContextError::new(Error::Canceled)));
        }
        if let Ok(res) = rx.try_recv() {
            break res;
        }
    };

    match result {
        Ok(_) => println!("[main] A completed"),
        Err(e) => println!("[main] A failed: {e}"),
    }
    cancel();
}

fn a(ctx: Context) -> Result<(), ContextError> {
    b(&ctx)
}

fn b(ctx: &Context) -> Result<(), ContextError> {
    c(ctx)?;
    d(ctx)?;
    Ok(())
}

fn c(_ctx: &Context) -> Result<(), ContextError> {
    // 无感知，纯睡眠 10s。
    std::thread::sleep(Duration::from_secs(10));
    d(_ctx)
}

fn d(ctx: &Context) -> Result<(), ContextError> {
    if let Some(err) = ctx.err() {
        return Err(err);
    }
    Ok(())
}
