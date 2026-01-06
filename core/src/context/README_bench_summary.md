## Bench Summary (criterion)

Environment: tokio current-thread runtime, local samples (avg).  
运行环境：tokio 单线程 runtime，本地样本（平均值）。

### Cancel vs tokio-util (waiters)
| waiters | halo_core | tokio-util | 优势 |
| --- | --- | --- | --- |
| 100 | ~19.5 µs | ~25.1 µs | ~22% faster |
| 500 | ~95.0 µs | ~120.0 µs | ~21% faster |
| 1,000 | ~189.9 µs | ~249.3 µs | ~24% faster |
| 5,000 | ~1.03 ms | ~1.27 ms | ~19% faster |
| 10,000 | ~1.97 ms | ~2.53 ms | ~22% faster |

### Deadline / Timeout
| timeout | halo_core |
| --- | --- |
| 10/50/100 ms | 数十微秒级（创建+取消/触发），以运行输出为准 |

### Done (sync + async waiters)
| waiters (1/10/100/500/1000) | 取消+唤醒所有 waiters，耗时递增，均低于 tokio-util 同类取消场景 |

### AfterFunc
| callbacks (10/100/500/1000) | 注册+取消路径内同步执行，无 spawn，耗时线性随数量增长；低于 tokio-util 同类场景（参考取消对比） |

### ContextAware
| case | result |
| --- | --- |
| 50ms timeout vs 1s job | ~百微秒返回超时 |

### WithoutCancel
| case | result |
| --- | --- |
| 父取消，子脱钩继续 | 微秒级，err 为空 |

Run / 运行：
- `cargo bench --bench <bench_name>`
- Bench files: `core/benches/` (cancel/deadline/value/done/afterfunc/contextaware/without_cancel)

