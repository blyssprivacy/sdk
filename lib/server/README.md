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
- vibes are off: feels like mmap was designed to map code into executable memory, because CPUs only wanna interact directly with RAM. Caching of mapped pages is best effort and works best with heavy reuse.


## Disk Setup

- Gotta use ext4, ZFS has too much overhead to achieve much beyond 1GB/s.
- `prepare_disks.sh` will find all NVMe devices matching a given prefix, format each of them as ext4, and mount them according to the standard structure `/mnt/flashpir/X` for X in {0..N-1}.
- Usage: `sudo ./prepare_disks.sh [-n] <disk_model>`, where the `-n` flag performs a read-only dry run.

### perf tuning

First, use fio to establish upper bound disk performance.

Need about 8 threads per NVMe device to saturate the disk, at 256KiB per read.

```
sudo fio --time_based --name=benchmark --size=16G --runtime=10 \
--filename=/mnt/flashpir/0/bench --ioengine=libaio --randrepeat=0 \
--iodepth=1 --direct=1 --invalidate=1 --verify=0 --verify_fatal=0 \
--numjobs=8 --rw=randread --blocksize=256k --group_reporting
```
Expect 6.5GB/s per Samsung 990 Pro 1TB @ PCIe 4.0 x4.

```
fio --time_based --name=benchmark --size=16G --runtime=10 \
--filename=/mnt/flashpir/0/wbench --ioengine=libaio --randrepeat=0 \
--iodepth=1 --direct=1 --invalidate=1 --verify=0 --verify_fatal=0 \
--numjobs=8 --rw=randwrite --blocksize=256k --group_reporting
```
Expect 6.5GB/s per Samsung 990 Pro 1TB @ PCIe 4.0 x4.



### FlashPIR benchmarking

```
cargo test --profile release-with-debug -- benchmark_sparse_db  --nocapture
```


```
cargo test --profile release-with-debug -- bench_mul --nocapture
```

```
sudo sysctl kernel.perf_event_paranoid=-1
sudo sh -c " echo 0 > /proc/sys/kernel/kptr_restrict"
mkfifo perf_ctl.fifo
exec {perf_ctl_fd}<>perf_ctl.fifo
echo $perf_ctl_fd

PERF_CTL_FD=$perf_ctl_fd perf record --delay=-1 --control fd:${perf_ctl_fd} -- cargo test --profile release-with-debug -- bench_mul

PERF_CTL_FD=$perf_ctl_fd perf stat --delay=-1 --control fd:${perf_ctl_fd} \
    -e task-clock,context-switches,cycles,instructions \
    -e cache-references,cache-misses \
    -e L1-dcache-load-misses,L1-dcache-loads \
    -e dTLB-load-misses,dTLB-loads \
    -e branch-load-misses,branch-loads \
    -e sse_avx_ops_retired.all \
    -e ls_tlb_flush.all \
    -e l2_pf_miss_l2_hit_l3.all \
    -- cargo test --profile release-with-debug -- bench_mul
```

### Disable swap
```
sudo swapoff -a
```

Manually delete swap from /etc/fstab, or:
```
sudo sed -i '/ swap / s/^\(.*\)$/#\1/g' /etc/fstab
```
Finally:
```
sudo rm /swap.img
```

