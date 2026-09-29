# Performance tuning

Status: draft

Throughput depends on core pinning, batch size, ring depth and the number of TX queues (RSS). The rate limiter combines a token bucket with a busy-poll and nanosleep hybrid, and the UI shows its accuracy. Use `wpg bench` to measure each backend and compare against the baseline in docs/perf.md. Reference figures are roughly 1-3 Mpps per core for afpacket and over 10 Mpps per core for DPDK.
