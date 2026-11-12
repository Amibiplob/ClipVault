# Performance measurements — 2.1.0 (with historical baselines)

No fixed RAM target was promised. Baseline was recorded before storage/UI optimization.

## Reproducible headless workload

Same host, debug Rust test build, 2,000 metadata records, 30 egui frames at 820×620, no screenshot rasterization, no native GPU/window/clipboard backend. Synthetic fixture text is about 1 KB per metadata record. Run only this explicit ignored workload:

```sh
cargo test --locked --offline performance_large_history -- --ignored --nocapture
```

| Measurement | 1.2.0 baseline | 2.0.0 initial optimized run |
|---|---:|---:|
| 30-frame render workload | 505 ms | 57 ms |
| Process VmHWM | 27,532 kB | 25,824 kB |
| Process VmRSS at sample | 27,532 kB | 25,632 kB |
| Test execution (excludes compilation) | 0.67 s | 0.14 s |

Logs: `target/v200-baseline-performance.log` and `target/v200-after-performance.log`. These are single samples, not statistically stable benchmarks. The workload render time fell about 89%; this does **not** establish native opening latency, idle CPU, or desktop RAM. Later implementation/check changes may affect measurements; final rerun is recorded separately in V2.0.0-VERIFICATION.md.

## Native measurements that remain unavailable

| Requested scenario | Before | After | Reason |
|---|---|---|---|
| Background RAM/CPU | Unmeasured | Unmeasured | Sandbox refuses normal launch while binding instance socket |
| Panel-open RAM | Unmeasured | Unmeasured | Native runtime sampling unavailable through shell |
| Large-image native RAM | Unmeasured | Unmeasured | No permitted measurable native session |
| Large-history native RAM/CPU | Unmeasured | Unmeasured | Headless workload above is partial evidence only |
| Native startup / panel-open latency | Unmeasured | Unmeasured | Screenshot start success is not a latency measurement |

The baseline launch used an isolated `/tmp/clipvault-v200-baseline` directory, not user history, and failed with `Operation not permitted (os error 1)` before GUI initialization. No before/after native memory or CPU savings are claimed. Measure on disposable real X11 and Wayland sessions before publishing those claims: process RSS/PSS and CPU deltas while hidden for 60 seconds; panel open; image preview; 10,000 clips; startup and 30 open/hide cycles. Record hardware, GPU/driver, distro/session, build profile, fixture dimensions/MIME bytes, sample interval and spread.

## Evidence-based changes

- Read database migration input in batches of 128 records. Keep at most 512 summary characters per item in memory; supported full MIME payloads stay on disk.
- Cache filtered metadata by storage revision/query/filter. A disk search table preserves the original stored search text beyond the summary. Metadata is still kept in memory for the configured item count; this is not a fully paged metadata store.
- Virtualize history rows; decode thumbnail/image data only when needed. At most 16 cached thumbnails and one full preview texture; full previews are scaled within 1600×1200 and decoded on a worker with a 128 MiB Rust decoder allocation limit. Native GIF/WebP fallback uses a scaled decoder; source bytes remain unchanged.
- Hide releases UI textures and selected text cache. Original image payloads load only on demand. One preview decode is in flight; no unbounded preview worker queue.
- Service/IPC events wake the UI. Hidden UI maintenance is every 30 seconds rather than every 250 ms; visible timestamp/toast refresh is once per second instead of every 100 ms.
- Desktop setting changes use GSettings/xfconf monitor processes rather than recurring subprocess reads. X11 clipboard event support is retained with a bounded fallback check; keyboard/paste waits are bounded state checks rather than long arbitrary sleeps.
- Geometry writes occur only when state changes. Clipboard ownership stays in the service while the UI is hidden. Single-instance locking remains per data directory.

Remaining performance work needs real profiles: clipboard activation loads a requested item's saved representations together on the clipboard worker, not the UI thread; metadata and long-text preview caching can be further optimized if measurements justify it. A large package size is acceptable; no unsupported download-size or native memory claim appears on the website.

Latest explicit workload rerun (during release compilation): 103 ms for 30 frames, VmHWM 25,544 kB / VmRSS 25,408 kB, test execution 0.25 s (target/v200-final-performance.log). Baseline/initial optimized samples above are kept unchanged. This remains a headless test-process measurement, not native desktop memory or CPU evidence.

## 2.1.0 sample — 2026-11-12

Same headless debug workload: 2,000 metadata records, 30 frames at 820×620:
**69 ms**, VmRSS **26,136 kB**, VmHWM **26,488 kB**, test execution **0.16 s**
(excludes compilation). Log: `target/v2.1.0/performance.log`. This single sample
is not evidence of an improvement over the older samples or a native memory/CPU
measurement. Native launch from the shell still fails at instance socket creation
with `Operation not permitted`; Craft Anything's separate native screenshot check
can show rendering but does not expose comparable idle profiling.

2.1.0 removes two repeated JSON serializations per settings frame, shares the
clear action's confirmation/cache cleanup logic, and releases retained image,
text and filtered-list resources after successful bulk deletion. Polling intervals,
clipboard ownership, native paste/shortcut integrations and migration code remain
unchanged. All 14 direct dependencies still have source callers, including X11
XInput/XKB/XFixes/XTEST features; none was removed on assumption.
