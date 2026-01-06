## 基准汇总（criterion）

环境：tokio 单线程 runtime，本地样本（平均值）。

### 环境信息（env_info bench）
| key | value |
| --- | --- |
| OS | windows |
| Arch | x86_64 |
| 逻辑 CPU | 16 |
| 物理 CPU | 8 |
| rustc | 1.92.0 (stable) |
| rustc commit | ded5c06cf21d2b93bffd5d884aa6e96934ee4234 |

### 取消（waiters 数量）
| waiters | halo_core |
| --- | --- |
| 100 | ~19.1 µs |
| 500 | ~94.8 µs |
| 1,000 | ~184.6 µs |
| 5,000 | ~0.98 ms |
| 10,000 | ~2.03 ms |

### 截止/超时
| timeout | halo_core |
| --- | --- |
| 10 ms | ~0.78 µs |
| 50 ms | ~0.86 µs |
| 100 ms | ~0.87 µs |

### Done（同步+异步 waiters）
| waiters | halo_core | 说明 |
| --- | --- | --- |
| n/a | n/a | 未在 20s 内完成（待补测） |

### AfterFunc
| callbacks | halo_core | 说明 |
| --- | --- | --- |
| 10 | ~1.38 µs | 取消路径内同步执行，无 spawn |
| 100 | ~8.96 µs |  |
| 500 | ~41.1 µs |  |
| 1,000 | ~78.7 µs |  |

### Value（WithValue）
| depth (1/4/16/64/256) | halo_core |
| --- | --- |
| 构建 + 查找 | ~76.3–76.6 µs |

### ContextAware
| 场景 | halo_core | 说明 |
| --- | --- | --- |
| 50ms 超时 vs 1s 任务 | ~64.4 ms | 返回超时 |

### WithoutCancel
| 场景 | halo_core | 说明 |
| --- | --- | --- |
| 父取消，子脱钩继续 | ~18.5 µs | err 为空 |

运行：
- `cargo bench --bench <bench_name>`
- 基准文件：`core/benches/`（cancel/deadline/value/done/afterfunc/contextaware/without_cancel）

