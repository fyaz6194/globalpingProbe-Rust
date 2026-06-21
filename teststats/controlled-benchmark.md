# Controlled Benchmark — Forced Identical Measurement

The [Performance](../README.md#performance) numbers in the main README come from probes
connected to the **live** Globalping API, sampled every 10s for 5 minutes. Those CPU "peak"
figures are opportunistic: whichever job the live network happened to dispatch during the
sampling window. Different job types (ping vs traceroute vs mtr) cost very different amounts
of CPU, so those peaks aren't directly comparable between runs.

This test fixes that by forcing the **exact same job** at both probes, in isolation from the
public network, and sampling tightly through the whole execution window.

## Method

1. A minimal Socket.IO server (`mock-dispatcher.js`, included in this folder) runs on the
   Oracle ARM64 host itself. It implements just enough of the Globalping wire protocol for a
   real probe to connect, report status, and receive a job — nothing else touches the
   public API.
2. Each probe (Rust, then Node.js) is started with its API-host env var pointed at
   `127.0.0.1:4000` instead of `api.globalping.io`, so it talks **only** to the mock.
3. Once the probe reports `"ready"` status, the mock dispatches one identical job:
   `ping 1.1.1.1`, 16 packets, ICMP. Same target, same packet count, same host, same network
   path, for both probes.
4. `docker stats` is polled in a tight loop (`--no-stream`, repeated as fast as the CLI allows)
   from before the job starts until the result is returned.

## Results

| Metric | Rust (arm64) | Node.js (arm64) |
|---|---|---|
| Job duration (dispatch → result) | 7.52 s | 7.52 s |
| CPU during job | 0.00–0.14% | 0.00–0.35% |
| RAM during job | ~2.2–2.5 MiB | ~56.6–57.5 MiB |

Raw dispatch log (Rust):
```
DISPATCH at 1782021980788
ACK      at 1782021980790   (+2ms)
RESULT   at 1782021988312   (7524ms later)
```

Raw dispatch log (Node.js):
```
DISPATCH at 1782022116883
ACK      at 1782022116924   (+41ms)
RESULT   at 1782022124405   (7522ms later)
```

## Finding

For a plain ping job, **CPU cost is negligible for both implementations** — neither rises
above ~0.4% at this sampling resolution. This means the 23% CPU spike recorded in the
opportunistic live-network test (see [`rust-arm64.md`](rust-arm64.md), sample s19) was **not**
caused by ping handling — it was a heavier job type (most likely `mtr` or `traceroute`, which
run many rounds of work) landing on the probe from the live network during that sampling
window. A single ping is cheap in both runtimes; the earlier peak numbers are not directly
comparable across job types.

## Caveats

- **RAM figures here are not comparable to the live-network baseline numbers.** The Rust probe
  connects to the mock over plain `http://`, skipping the TLS/OpenSSL handshake it normally
  does against `https://api.globalping.io` — this alone accounts for several MB of the
  difference between this test's ~2.3 MiB baseline and the live-network ~5.4 MiB baseline.
  The Node.js side shows the opposite shift (higher here than live) likely due to reconnect-
  attempt noise after the mock process exits post-job. Use the live-network numbers in the
  main README for RAM comparisons; use this test only for the CPU-during-identical-job finding.
- Ack latency (Rust: 2ms, Node.js: 41ms) is a single-sample data point, not a statistically
  robust benchmark — provided for reference, not as a performance claim.
