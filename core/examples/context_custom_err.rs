use core::context::{
    Background, Context, ContextAware, ContextError, Error, WithTimeout, WithValue,
};
use std::time::Duration;

// 使用自定义消息构造 ContextError（Error::new_err）。
async fn work(ctx: Context) -> Result<(), ContextError> {
    let val_arc = ctx
        .value(&"user_id")
        .ok_or_else(|| Error::new("missing user_id"))?
        .downcast::<u64>()
        .map_err(|_| Error::new("user_id type mismatch"))?;
    println!("value: {}", *val_arc);
    tokio::time::sleep(Duration::from_secs(5)).await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), ContextError> {
    let (ctx, cancel) = WithTimeout(Background(), Duration::from_secs(1));
    let ctx = WithValue(ctx.clone(), "user_id", 42u16);

    // 也可以在入口处检查并返回自定义错误信息。
    let val_arc = ctx
        .value(&"user_id")
        .ok_or_else(|| Error::new("missing user_id"))?
        .downcast::<u16>()
        .map_err(|_| Error::new("user_id type mismatch"))?;
    assert_eq!(*val_arc, 42);

    let res = ContextAware(ctx.clone(), work(ctx.clone())).await;
    cancel();

    match res {
        Ok(_) => println!("work ok"),
        Err(e) => println!("work failed: {e}"),
    }

    Ok(())
}
