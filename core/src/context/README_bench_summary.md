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
| 10/50/100 ms | tens of microseconds (create + cancel/trigger), see run output |

### Done (sync + async waiters)
| waiters (1/10/100/500/1000) | Cancel + wake all waiters; increases with n; below tokio-util in similar scenarios |

### AfterFunc
| callbacks (10/100/500/1000) | Register + execute in cancel path, no spawn; cost scales linearly; below tokio-util (see cancel comparison) |

### ContextAware
| case | result |
| --- | --- |
| 50ms timeout vs 1s job | ~hundreds of microseconds, returns timeout |

### WithoutCancel
| case | result |
| --- | --- |
| parent canceled, child detached continues | microsecond-level, err is None |

Run:
- `cargo bench --bench <bench_name>`
- Bench files: `core/benches/` (cancel/deadline/value/done/afterfunc/contextaware/without_cancel)

