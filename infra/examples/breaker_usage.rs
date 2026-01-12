use infra::breaker::{Breaker, BreakerConfig, BreakerPolicy, ExecuteError, Reject};
use infra::context::Background;

fn main() -> anyhow::Result<()> {
    let ctx = Background();

    // 1) RollingWindow（10s/40桶）采集 + Google SRE(eq2101) 判定是否拒绝
    let brk = Breaker::new("demo", BreakerConfig::default())?;
    let v: i32 = brk.execute(&ctx, || Ok(42))?;
    println!("execute ok: {v}");

    // 2) execute_with_acceptable：错误可接受（统计视为成功，但返回值仍是 Err）
    let _: Result<(), _> =
        brk.execute_with_acceptable(&ctx, || Err(anyhow::anyhow!("logical error")), |_e| true);

    // 3) 固定时间窗口采集 + Google SRE(eq2101) 判定是否拒绝
    let fixed = Breaker::new(
        "demo-fixed",
        BreakerConfig::FixedWindow {
            window: std::time::Duration::from_secs(10),
            google: None,
        },
    )?;
    let hold = fixed.allow(&ctx)?; // 占住一个 permit（演示降级写法）

    let v = fixed.execute_with_fallback(
        &ctx,
        || Ok::<_, anyhow::Error>(1),
        |rej: Reject| match rej {
            Reject::Open { .. } => Ok(0),
            _ => Err(ExecuteError::Rejected(rej)),
        },
    )?;
    println!("fallback value: {v}");

    // 释放占用（否则 Drop 会按 fail("dropped") 结算）
    hold.success();

    Ok(())
}
