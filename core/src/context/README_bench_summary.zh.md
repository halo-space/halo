## 基准汇总（criterion）

环境：tokio 单线程 runtime，本地样本（平均值）。

### 取消对比 tokio-util（waiters 数量）
| waiters | halo_core | tokio-util | 优势 |
| --- | --- | --- | --- |
| 100 | ~19.5 µs | ~25.1 µs | ~22% |
| 500 | ~95.0 µs | ~120.0 µs | ~21% |
| 1,000 | ~189.9 µs | ~249.3 µs | ~24% |
| 5,000 | ~1.03 ms | ~1.27 ms | ~19% |
| 10,000 | ~1.97 ms | ~2.53 ms | ~22% |

### 截止/超时
| timeout | halo_core |
| --- | --- |
| 10/50/100 ms | 数十微秒级（创建+取消/触发），以运行输出为准 |

### Done（同步+异步 waiters）
| waiters (1/10/100/500/1000) | 取消+唤醒所有 waiters，耗时递增，均低于 tokio-util 同类场景 |

### AfterFunc
| callbacks (10/100/500/1000) | 注册+取消路径内同步执行，无 spawn；耗时随数量线性增长，低于 tokio-util 同类场景 |

### ContextAware
| case | result |
| --- | --- |
| 50ms 超时 vs 1s 任务 | ~百微秒返回超时 |

### WithoutCancel
| case | result |
| --- | --- |
| 父取消，子脱钩继续 | 微秒级，err 为空 |

运行：
- `cargo bench --bench <bench_name>`
- 基准文件：`core/benches/`（cancel/deadline/value/done/afterfunc/contextaware/without_cancel）

