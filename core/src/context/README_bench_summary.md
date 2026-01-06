## Bench Summary (criterion)

Environment: tokio current-thread runtime, local samples (avg).  
运行环境：tokio 单线程 runtime，本地样本（平均值）。

### Environment Info (env_info bench)
| key | value |
| --- | --- |
| OS | windows |
| Arch | x86_64 |
| Logical CPUs | 16 |
| Physical CPUs | 8 |
| rustc | 1.92.0 (stable) |
| rustc commit | ded5c06cf21d2b93bffd5d884aa6e96934ee4234 |

### Cancel (waiters)
| waiters | halo_core |
| --- | --- |
| 100 | ~19.1 µs |
| 500 | ~94.8 µs |
| 1,000 | ~184.6 µs |
| 5,000 | ~0.98 ms |
| 10,000 | ~2.03 ms |

### Deadline / Timeout
| timeout | halo_core |
| --- | --- |
| 10 ms | ~0.78 µs |
| 50 ms | ~0.86 µs |
| 100 ms | ~0.87 µs |

### Done (sync + async waiters)
| waiters | halo_core | note |
| --- | --- | --- |
| n/a | n/a | not completed under 20s (bench pending) |

### AfterFunc
| callbacks | halo_core | note |
| --- | --- | --- |
| 10 | ~1.38 µs | cancel-path sync exec |
| 100 | ~8.96 µs |  |
| 500 | ~41.1 µs |  |
| 1,000 | ~78.7 µs |  |

### Value (WithValue)
| depth (1/4/16/64/256) | halo_core |
| --- | --- |
| build + lookup | ~76.3–76.6 µs |

### ContextAware
| case | halo_core | note |
| --- | --- | --- |
| 50ms timeout vs 1s job | ~64.4 ms | returns timeout |

### WithoutCancel
| case | halo_core | note |
| --- | --- | --- |
| parent canceled, child detached continues | ~18.5 µs | err is None |

Run:
- `cargo bench --bench <bench_name>`
- Bench files: `core/benches/` (cancel/deadline/value/done/afterfunc/contextaware/without_cancel)

