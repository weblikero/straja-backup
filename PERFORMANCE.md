# Resource use

Straja favors foreground applications over backup throughput. It does not change priorities or settings of other applications.

- The dedicated agent/service enters Windows background processing mode (CPU, I/O and memory scheduling). If Windows rejects that mode, it tries below-normal CPU priority; a failure is logged. The GUI and tray remain interactive.
- Agent and GUI Tokio executors use two async workers and at most four blocking workers. The tray uses a single-thread async runtime on its monitoring thread.
- Restic runs below normal CPU priority with `GOMAXPROCS=2`, `RESTIC_READ_CONCURRENCY=1` and a `GOMEMLIMIT=512MiB` soft target, applied only to the child process. Large repositories may exceed the memory target; this is not a hard process RAM cap.
- GUI polling slows to ten seconds while idle (five seconds disconnected); active operations retain two-second updates. The tray polls at fifteen seconds while idle and two seconds while active. Scheduled operations may take up to those idle intervals to appear. Explicit GUI operations refresh immediately.
- The tray receives the plan count, active operation and latest finished outcome, without transferring all plans and history. Indexed history queries avoid repeated sorting. Old agents fall back to the original request so their version warning remains available. Unchanged icons/tooltips are not resent to Explorer.
- ZIP and TAR.GZ use compression level 1; 7z uses single-thread LZMA2 level 1. New archives can be larger. Existing archives still restore; formats and integrity verification remain unchanged.
- Archive creation passes its manifest back instead of cloning it and iterates file paths on demand. Archive verification reuses a 256 KiB buffer across entries.
- The source read lock is released after copying and checking source metadata, before flushing and verifying the destination. Files open for writing may still be unavailable. Since 0.6.0, VSS can read supported local NTFS sources from a snapshot. Without VSS, short read locks and filesystem errors remain possible; live database consistency is not guaranteed.

The agent still runs only one backup/restore/check at a time. Its per-account pipe prevents two workers owning the same database. Verification and retention safety are preserved. Zero contention or compatibility with every application cannot be guaranteed: disk speed, antivirus scans, file counts, source changes and repository size still matter.

## Large manifests — 0.6.2

The old 64 MiB limit applied to pretty-printed JSON written after source copying, and separately to the raw external and archived manifest buffers. A source set with many paths could therefore copy for hours and fail before compression. All manifest paths now share a 512 MiB serialized-size limit. Compact JSON is streamed to a buffered file; parsing runs on blocking workers from a bounded buffered reader. Archive verification hashes the external metadata and the archived metadata incrementally and compares both length and SHA-256. It does not allocate either complete byte array. Archive creation keeps the manifest returned by the copy stage instead of parsing it again, and archive restore shares one parsed index using `Arc` instead of cloning it for each verification pass.

Scanning accumulates a conservative JSON-size budget using borrowed paths, maximum numeric file sizes, checksums and timestamp margin. An over-budget plan fails before file content copying; staging/marker creation and snapshot creation may already have occurred, and owned snapshot cleanup still runs. Existing partial copies are preserved. The parsed entry list, validation sets and archive lookup tables still scale with the number/length of paths; this is not constant-memory indexing or a hard RAM cap. The 512 MiB limit is not a maximum file-data size. Split extremely large source sets into separate plans.

## Small-file buffers — 0.6.1

Plain copying, pre-archive staging, plain verification and restoration now reuse a 256 KiB buffer across files. The previous copy and digest functions each allocated and zeroed a new 1 MiB buffer for every file, including tiny files. SHA-256 destination rereads, durable flushes, source sharing and snapshot cleanup are unchanged. Compression and Restic budgets are unchanged.

A live diagnostic during a VSS TAR.GZ backup found the source C: disk continuously busy in five samples, concurrent PostgreSQL I/O and 52,542–90,894 page faults/s in the agent. Page faults include soft faults and must not be interpreted as disk reads or proof of insufficient RAM. About 26 GiB was available in the initial sample. AnyDesk also logged DNS and UI errors; the report does not establish that Straja caused its failure. No installed process, schedule or backup was stopped or changed. A synthetic background-priority baseline probe could not finish within its five-minute budget under that concurrent load and was stopped; no before/after speedup or CPU percentage reduction is claimed. Ten focused plain/archive integration tests passed, including mixed-size buffer reuse and restoration.

## Interactive rendering — 0.5.2

The installed release now prefers Iced/wgpu GPU rendering, with tiny-skia used if GPU initialization fails. Vsync stays enabled and ordinary windows render on demand. Only the explicit isolated scroll probe drives continuous frames; the installed release contains no probe. Background backup priorities and concurrency limits are unchanged. GPU rendering trades additional process/graphics memory for lower CPU time and smoother scrolling.

The static logo handle is initialized once rather than hashed/allocated on every view rebuild. Exact translated messages use a hash index; dynamic templates retain the same specificity order. Existing seven-language translation tests still pass. The old `Localization preview` screenshot was from an unoptimized debug fixture; use an optimized fixture when assessing responsiveness.

### Isolated scroll comparison

Same optimized 0.5.2 fixture, 1120 × 780 window, 100 fictitious History rows, 120 warm-up redraw events followed by 180 sampled events. Each redraw moves the same scrollable by 40 logical pixels, reversing direction periodically. One run per backend on this Windows PC, with no real agent, settings, network, plans or backups. GPU was forced with `ICED_BACKEND=wgpu`; software with `ICED_BACKEND=tiny-skia`, only in the test child environment. This compares backends within the new build; it is not a measured 0.5.0-versus-0.5.2 comparison or a guarantee for other computers.

| Metric | Software (tiny-skia) | GPU (wgpu) |
|---|---:|---:|
| Mean redraw interval | 31.05 ms (~32 fps) | 16.67 ms (~60 fps) |
| 95th percentile redraw interval | 36.21 ms | 17.29 ms |
| Maximum sampled interval | 38.16 ms | 17.70 ms |
| Total process CPU time, including initialization and warm-up | 9.66 s | 1.64 s |
| Peak sampled process working set | 53.29 MiB | 202.11 MiB |
| Peak sampled private memory | 33.60 MiB | 261.14 MiB |

Working set/private memory are different accounting measures; neither measures total dedicated GPU VRAM. Memory was sampled every 200 ms. Frame events approximate application redraw cadence, not input-to-photon latency or GPU timestamp measurements. Foreground activity, display refresh rate, drivers and window size can affect the result. The benchmark intentionally uses the maximum current history size. A separate smoke test disabled usable GPU backends only in the test child (`WGPU_BACKEND=noop`, with automatic renderer selection); the same scroll fixture completed through automatic software fallback. This simulated initialization failure, not a test on a physically GPU-less PC. Its timing is excluded from the comparison because compilation was running concurrently.

Reproduce locally after building an isolated optimized fixture (never distribute this executable as the installed release):

```powershell
.\scripts\cargo.ps1 build --locked --release --features ui-preview --bin straja
Copy-Item target\release\straja.exe test-output\straja-scroll-preview-0.5.2.exe
.\scripts\measure-scroll.ps1 -Renderer wgpu
.\scripts\measure-scroll.ps1 -Renderer tiny-skia
```

The runner stops only its own fixture on a 120-second timeout. Reports: `test-output/scroll-wgpu-8a00e7379e1b4ff8bc2a3cbc74bfee15.json` and `test-output/scroll-tiny-skia-813227ec6313485aa24f6a2e782b9a1f.json`. Production packaging builds without `ui-preview`. A temporary, process-local `ICED_BACKEND=tiny-skia` override can help diagnose graphics-driver issues; it is not written into the user's environment by Straja.

## Background workload measurements — 0.4.2

Windows x64, debug builds, synthetic data, one run per version. Small regression probes, not production benchmarks or promised limits.

| Probe | 0.4.1 | 0.4.2 |
|---|---:|---:|
| Idle-agent: requests in 20 seconds | 20 | 4 |
| Idle-agent: response bytes | 9,207,080 | 921,006 |
| Idle-agent: maximum observed threads | 33 | 3 |
| Idle-agent: peak sampled working set | 14.20 MiB | 12.65 MiB |
| Idle-agent: sampled cumulative CPU | 1.08 s | 0.50 s |
| Archive roundtrip: peak working set | 25.62 MiB | 18.72 MiB |
| Archive roundtrip: sampled cumulative CPU | 2.25 s | 1.22 s |

The idle probe runs an isolated agent with 100 synthetic completed history entries and emulates GUI/tray requests. Actual GUI rendering and the tray process are excluded. CPU includes synthetic database setup. Sampling: 500 ms. The archive probe runs the same `all_archive_formats_roundtrip_compress_and_keep_two` test, including verification, restore and retention for ZIP/TAR.GZ/7z; sampling: 20 ms. Compression was measured before the final manifest-clone removal. No user files or real plans were used.

Probe: `examples/resource_probe.rs`. Local evidence: `test-output/probe-{before,after}.json`, `test-output/probe-{before,after}-output.txt`, `test-output/compression-{before,after}.json`. Logs are excluded from Git/distributions. Debug measurements do not establish release consumption, encrypted-backup peaks or behavior on other PCs.

## References

- Windows: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setpriorityclass
- Restic: https://restic.readthedocs.io/en/stable/047_tuning_parameters.html
- Go soft memory target: https://go.dev/doc/gc-guide
