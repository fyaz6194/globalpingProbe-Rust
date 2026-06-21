# Rust Probe — amd64 (WSL2 / x86_64)

**Image:** `globalping-probe:dev` (built from latest source)  
**Host:** WSL2 Ubuntu on Windows 11 (15.54 GiB RAM)  
**Network:** `--network host`  
**Duration:** 5 minutes, 30 samples × 10 s interval  
**Date:** 2026-06-21

## Raw Samples

| Sample | CPU % | Memory |
|--------|-------|--------|
| s1  | 1.26% | 23.08 MiB |
| s2  | 0.00% | 16.30 MiB |
| s3  | 0.00% | 15.44 MiB |
| s4  | 0.00% | 15.45 MiB |
| s5  | 0.02% | 19.58 MiB |
| s6  | 0.00% | 19.32 MiB |
| s7  | 0.00% | 19.34 MiB |
| s8  | 0.00% | 19.34 MiB |
| s9  | 0.02% | 19.22 MiB |
| s10 | 0.00% | 19.22 MiB |
| s11 | 0.02% | 19.29 MiB |
| s12 | 0.03% | 18.73 MiB |
| s13 | 0.02% | 18.96 MiB |
| s14 | 0.00% | 18.96 MiB |
| s15 | 0.00% | 18.81 MiB |
| s16 | 0.04% | 19.05 MiB |
| s17 | 0.01% | 19.05 MiB |
| s18 | 0.00% | 18.87 MiB |
| s19 | 0.02% | 18.87 MiB |
| s20 | 0.02% | 18.80 MiB |
| s21 | 0.03% | 18.80 MiB |
| s22 | 0.00% | 19.54 MiB |
| s23 | 0.02% | 19.39 MiB |
| s24 | 0.00% | 19.39 MiB |
| s25 | 0.00% | 19.07 MiB |
| s26 | 0.00% | 19.32 MiB |
| s27 | 0.00% | 19.32 MiB |
| s28 | 0.00% | 19.68 MiB |
| s29 | 0.00% | 19.69 MiB |
| s30 | 0.02% | 19.68 MiB |

## Summary

| Metric | Value |
|--------|-------|
| CPU avg (idle/connected) | ~0.01% |
| CPU peak (startup) | 1.26% |
| RAM min | 15.44 MiB |
| RAM max | 23.08 MiB (startup spike) |
| RAM steady state avg | ~19.1 MiB (s5–s30) |

> **Note:** WSL2 runs inside a Hyper-V VM. Docker memory stats in WSL2 include
> some virtual-machine overhead and may read higher than on bare-metal Linux.
> The Oracle ARM server provides more accurate bare-metal figures.
