use infra::storage::redis::{Conf, new_redis};
use redis::AsyncCommands;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 构造配置：单节点示例；如需集群，kind 设置为 "cluster"，host 用逗号分隔多个节点。
    let conf = Conf {
        host: "127.0.0.1:6379".into(),
        kind: "node".into(),
        non_block: false,
        ..Default::default()
    };

    // new_redis 直接返回统一的异步连接枚举，可用 redis-rs 的 AsyncCommands/AsyncTypedCommands。
    let mut conn = new_redis(conf).await?;

    // 基础命令
    let _: () = conn.set("halo:demo:key", "hello").await?;
    let v: String = conn.get("halo:demo:key").await?;
    println!("get halo:demo:key => {v}");

    // mset/mget 也直接可用
    let _: () = conn
        .mset(&[("halo:demo:k1", 1), ("halo:demo:k2", 2)])
        .await?;
    let nums: Vec<i32> = conn.mget(&["halo:demo:k1", "halo:demo:k2"]).await?;
    println!("mget => {:?}", nums);

    Ok(())
}
