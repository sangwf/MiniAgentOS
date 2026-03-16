# MiniOS Spec (AArch64 / QEMU virt)

## 1. Goals
- Build a minimal AArch64 OS kernel that runs in QEMU `virt`.
- Implement networking with virtio-net and send HTTPS requests from kernel.
- Final demo: POST/GET to Twitter API via HTTPS (mbedTLS in-kernel).

## 2. Scope & Assumptions
- Target: `qemu-system-aarch64 -machine virt -cpu cortex-a53 -nographic`.
- Language: Rust `no_std` + small AArch64 ASM + C (mbedTLS).
- Network stack: minimal in-kernel stack for now; smoltcp is optional later.
- TLS: mbedTLS C library via FFI.
- SOCKS5 proxy (optional) for HTTPS via host at `10.0.2.2:10808`.
- Twitter auth: OAuth 1.0a user context (HMAC-SHA1), secrets loaded at build time from `xsecret.txt` (gitignored).
- QEMU user-net NAT: guest IP `10.0.2.15`, gateway `10.0.2.2`, DNS `10.0.2.3` (for external access).

## 3. Milestones (Spec Coding)
1) Boot + UART
   - Boot into Rust `kmain`, print `hello world`.
2) Basic kernel facilities
   - Timer, allocator, MMIO helpers.
3) Virtio-net bring-up (current)
   - Detect virtio-net, init queues, RX/TX works.
4) Network stack
   - Integrate smoltcp, IP config (static or DHCP), TCP + HTTP.
5) TLS
   - Port mbedTLS, implement HTTPS client in kernel (TLS 1.2).
6) Twitter API
   - Send request and print response.

## 4. Architecture
- Boot: `_start` ASM -> `kmain`.
- Drivers: PL011 UART, virtio-mmio, virtio-net.
- Memory: bump allocator (1 MiB for now).
- Net: virtio-net device + minimal in-kernel stack (ARP/DNS/UDP/TCP/ICMP/HTTP).
- TLS: mbedTLS (C), Rust FFI wrappers.

## 5. Interfaces / Modules
- `uart`: init, write byte/string.
- `timer`: read cntfrq/cntpct, delay.
- `mmio`: read/write + barriers. Prefer inline-asm loads/stores for device/DMA memory.
- `virtio`: mmio registers + features.
- `net`: virtio-net queue mgmt + TX/RX.
- `allocator`: bump allocator.

## 5.1 DMA / MMIO Access Rules
- Do NOT use `core::ptr::read_volatile` on DMA buffers or device-owned rings. Rust UB precondition checks can panic/abort in kernel.
- Use `mmio::read8/read16/read32` and `mmio::store8/store16/store32` (inline asm) for all device MMIO, virtio rings, and RX/TX buffers.
- If higher-level parsing needs bytes/words, read via `mmio::*` into local variables first.

## 6. Acceptance Tests
- M1: QEMU prints `hello world`.
- M2: Timer logs ticks.
- M3: virtio-net TX used ring updates.
- M3.1: ICMP echo request/reply works with 10.0.2.2.
- M3.2: UDP echo service replies via hostfwd (port 5555).
- M3.3: DNS resolve + HTTP GET over TCP prints response bytes.
- M4: HTTP GET over TCP returns 200 header.
- M5: HTTPS handshake + HTTP over TLS (response bytes printed).
- M6: Twitter API returns 201 (tweet created).

## 7. Current Status (Summary)
- Boot + UART OK.
- Allocator OK.
- virtio-net detected, queues init OK.
- Legacy virtio-net path: TX used ring updates; RX used ring updates.
- RX payload access works only via asm MMIO reads (see Notes below).
- ICMP ping to 10.0.2.2 works (request + reply parsed).
- UDP service on port 5555 replies with `WalleOS t=<ms> c=<count>` prefix and simple commands (`ping`, `mac`, `echo ...`).
- UDP command accepts URLs (e.g., `http://neverssl.com/`, `https://example.com/`) to trigger fetch; headers are chunked over UDP (`http chunk`), body chunks are sent via `body chunk` (or `json chunk` for JSON). UART console prints headers/body/json directly.
- HTTPS can use SOCKS5 proxy (Clash Verge default `10.0.2.2:10808`).
- HTTP redirects (301/302) are followed automatically (up to 3 hops).
- UART console accepts URLs directly (prompt `> `), no `test.py` required.
- OAuth 1.0a signing implemented; `tweet <text>` posts to `https://api.twitter.com/2/tweets` and returns 201.
- OAuth time can be set via `time <unix_seconds>`; Date header parsing updates the time offset when present.
- `sync` triggers an HTTPS GET to fetch Date and update OAuth time.
- DNS (UDP) resolution works; HTTP/1.1 GET over TCP prints response bytes via host NAT.
- HTTPS (TLS 1.2) via mbedTLS works; `https://example.com/` returns 200 and body bytes.
- Plain HTTP: `neverssl.com` succeeds reliably; `example.com` can fail due to Cloudflare behavior (ACK/FIN without payload).

## 8. Risks / Notes
- Virtio legacy vs modern path differences.
- TLS stack complexity in no_std.
- TLS currently disables certificate verification and allows Curve25519 unconditionally (temporary; tighten later).
- Twitter API tokens + rate limits.
- Bare-metal rule: do not use `core::ptr::read_volatile` for DMA/MMIO buffers. It triggers Rust UB precondition checks and can panic/abort in kernel. Use `mmio::read8/read16/read32` (inline asm) for RX buffers, used rings, and other device-owned memory.
