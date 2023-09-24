# spiral-server

Rust server for the [Spiral PIR scheme](https://eprint.iacr.org/2022/368), written by [Blyss](https://blyss.dev). More details are in the [repo](https://github.com/blyssprivacy/sdk).

## MMAP

- Deduplicate data between worker process memory and kernel page cache.
- Async IO "for free" via madvise; would alternatively need to issue prefetch reads in a separate thread.
- Very high utilization of machine RAM without OOM risk.
- Reduced syscall overhead (one context switch per read(), followed by memcpy(); vs. one-time mmap() per file. So, save a microsec or two per random read).