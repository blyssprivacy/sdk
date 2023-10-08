# spiral-server

Rust server for the [Spiral PIR scheme](https://eprint.iacr.org/2022/368), written by [Blyss](https://blyss.dev). More details are in the [repo](https://github.com/blyssprivacy/sdk).

## MMAP

### Pros

- Deduplicate data between worker process memory and kernel page cache.
- Async IO "for free" via madvise; would alternatively need to issue prefetch reads in a separate thread.
    - nvm, madvise(WILLNEED) is actually a blocking call 🤷
- Very high utilization of machine RAM without OOM risk.
- Reduced syscall overhead (one context switch per read(), followed by memcpy(); vs. one-time mmap() per file. So, save a microsec or two per random read).

### Cons

[Are you sure you want to use MMAP](https://db.cs.cmu.edu/mmap-cidr2022/)
- "TLB shootdowns" bottleneck throughput; every page swap requires TLB sync.
- vibes are off: feels like mmap was designed to map code into executable memory
- 