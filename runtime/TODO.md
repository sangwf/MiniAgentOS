# MiniOS Spec Coding TODO

## Spec coding status
- Spec coding: **in progress**
- Current milestone: **X/Twitter POST (OAuth 1.0a)** over HTTPS

## Completed
- Bootable AArch64 Rust kernel on QEMU virt
- PL011 UART output + hello world
- ARM Generic Timer support (present, not active)
- Basic MMIO helpers with inline-asm barriers
- Simple bump allocator (1 MiB)
- Virtio-mmio scan: finds virtio-net device
- Virtio-mmio legacy init: queue setup (PAGE_SIZE, QUEUE_ALIGN, QUEUE_PFN)
- DNS (UDP) resolve via user-net
- TCP three-way handshake (minimal)
- HTTP/1.1 GET (prints response headers/body chunks)
- HTTPS GET via mbedTLS (TLS 1.2) prints response bytes
- OAuth 1.0a signing (HMAC-SHA1 + base64 via mbedTLS)
- POST `/2/tweets` via `tweet <text>`

## Current behavior (latest run)
- Detects virtio-net device (MMIO version = 1 legacy)
- Initializes RX/TX virtqueues successfully
- Sends ARP request (virtio-net header + Ethernet/ARP)
- TX used ring updates
- RX used ring updates (len observed)
- Legacy registers `QUEUE_NUM`/`QUEUE_ALIGN` appear write-only (reads return 0)
- D/I caches are disabled in boot to avoid DMA coherency issues
- RX payload access must use inline-asm `mmio::read*` (avoid `read_volatile` UB checks)
- ICMP echo request/reply works (ping 10.0.2.2)
- UDP echo service works on port 5555 (hostfwd), with simple commands and interactive client
- UDP URL command triggers DNS+HTTP fetch and prints response on UART
- UART console accepts URLs directly (prompt `> `)
- DNS resolve via 10.0.2.3
- TCP connect + HTTP/1.1 GET prints response bytes (headers + body)
- Plain HTTP: `neverssl.com` succeeds reliably; `example.com` often fails (Cloudflare ACK/FIN)
- HTTPS: `https://example.com/` succeeds; response bytes printed on UART
- OAuth time set via `time <unix_seconds>`; Date header parsing updates time
- `sync` command fetches Date header to set OAuth time
- `tweet <text>` posts to `api.twitter.com` and returns 201 + JSON response

## Key recent debug prints (expected)
- "virtio dev @ ... id: 1 ver: 1"
- "virtio-net queues ready"
- "virtio-net tx notify"
- "virtio-net tx used idx: ..."
- "virtio-net rx used idx: ..."
- "virtio-net rx icmp echo reply"
- "virtio-net udp reply sent"

## Files changed recently
- `src/main.rs` (virtio probe, debug prints, TX path)
- `src/virtio.rs` (legacy/modern feature handling)
- `src/net.rs` (virtio-net queue init, TX dummy, debug)
- `src/mmio.rs` (asm stores, barriers)
- `Makefile` (run-net target)

## Open issues / next steps
1) **Harden RX parsing**
   - Continue strict bounds checks for IP/UDP lengths.
   - Consider reusing/rotating RX buffers for sustained throughput.

2) **Consider modern virtio path**
   - Investigate whether modern virtio-mmio is exposed; if so, use descriptor/avail/used addresses via 0x080+ registers.

3) **DTB parsing (optional)**
   - DTB parsing module exists but currently not wired; base is hardcoded at 0x0a000000

4) **Networking stack**
   - Decide between keeping minimal stack or integrating smoltcp
   - Add TCP state machine, retransmits, windowing
   - Harden HTTPS: verify certificates, tighten curve policy, and remove temporary Curve25519 bypass
   - Improve TLS memory behavior (allocator and peak usage tracking)

## How to run
- Build: `make build`
- Run (with virtio-net): `make run-net`
- Run external HTTP demo: `make run-net-legacy` (or HTTPS via UDP/console)
- Run UDP echo demo: `make run-net-legacy-fwd` then `python3 test.py`
- Run UART console demo: `make run-net-legacy-fwd` then type `http://neverssl.com/` or `https://example.com/` in the QEMU console
