# Rust Probe — arm64 (Oracle Cloud, Mumbai)

**Image:** `globalping-probe:arm64` (built natively on Oracle from latest source)  
**Host:** Oracle Cloud ARM64, 1 OCPU / 5.763 GiB RAM, Mumbai  
**Network:** `--network host`  
**Duration:** 5 minutes, 30 samples × 10 s interval  
**Date:** 2026-06-21

## Raw Samples

| Sample | CPU % | Memory |
|--------|-------|--------|
| s1  | 0.02% | 5.414 MiB |
| s2  | 0.00% | 5.164 MiB |
| s3  | 0.02% | 5.473 MiB |
| s4  | 0.00% | 5.359 MiB |
| s5  | 0.00% | 5.531 MiB |
| s6  | 0.03% | 5.527 MiB |
| s7  | 0.00% | 5.277 MiB |
| s8  | 0.03% | 5.527 MiB |
| s9  | 0.02% | 5.387 MiB |
| s10 | 0.00% | 5.285 MiB |
| s11 | 0.00% | 5.285 MiB |
| s12 | 0.00% | 5.535 MiB |
| s13 | 0.03% | 5.535 MiB |
| s14 | 0.00% | 5.578 MiB |
| s15 | 0.00% | 5.289 MiB |
| s16 | 0.00% | 5.539 MiB |
| s17 | 0.00% | 5.289 MiB |
| s18 | 0.02% | 5.289 MiB |
| s19 | 23.03% | 5.602 MiB ← measurement received |
| s20 | 0.02% | 5.344 MiB |
| s21 | 0.00% | 5.547 MiB |
| s22 | 0.00% | 5.547 MiB |
| s23 | 0.02% | 5.547 MiB |
| s24 | 0.00% | 5.547 MiB |
| s25 | 0.02% | 5.602 MiB |
| s26 | 0.00% | 5.305 MiB |
| s27 | 0.00% | 5.305 MiB |
| s28 | 0.00% | 5.305 MiB |
| s29 | 0.00% | 5.305 MiB |
| s30 | 0.03% | 5.609 MiB |

## Summary

| Metric | Value |
|--------|-------|
| CPU idle avg | ~0.01% |
| CPU peak (measurement) | 23.03% |
| RAM min | 5.164 MiB |
| RAM max | 5.609 MiB |
| RAM steady state avg | ~5.43 MiB |

> CPU spikes (here 23%) occur only during active measurements (ping, traceroute,
> DNS, HTTP). Idle CPU between measurements is effectively 0%.
