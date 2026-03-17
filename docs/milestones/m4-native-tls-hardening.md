# M4 Guest-Native TLS Hardening Plan

## Goal

Restore confidence in the guest-native OpenAI HTTPS path so the default live
`Goal >` path can work without depending on a host-side OpenAI bridge.

This document is not about the already-fixed M5 plain HTTP bridge transport.
It is about the original guest-native path:

- guest TCP connect
- optional guest SOCKS5 proxy connect
- guest mbedTLS handshake
- guest HTTPS request and response handling

## Why This Needs Its Own Plan

The current guest-native TLS path is neither fully broken nor reliably healthy.
Live reproduction on 2026-03-16 showed:

- a fresh-session OpenAI request can succeed
- a later request in the same guest can fail with `MBEDTLS_ERR_SSL_INVALID_MAC`
- follow-up retries can then fail with `MBEDTLS_ERR_SSL_FATAL_ALERT_MESSAGE`
- later retries may degrade further into `tcp connect timed out`

That pattern points to connection or record lifecycle fragility, not just DNS
or a missing proxy.

## Root Cause Update: Local Second-Request Corruption Was Stack Overflow

The highest-signal local repro is now understood much better.

MiniAgentOS originally linked the guest stack as only `16 KiB` in
[runtime/linker.ld](/Users/sangwf/code/MiniAgentOS/runtime/linker.ld), placed
immediately after `.bss`:

- `__bss_end`
- `__stack_bottom`
- `__stack_top`

In practice, the mbedTLS handshake plus the added TLS diagnostics can exceed
that budget. Because the stack grows downward, overflowing below
`__stack_bottom` writes directly back into the tail of `.bss`, which is where
the small fetch and agent globals live.

That matches the corrupted runtime state observed during the failing repro:

- `FETCH_PATH_LEN` turning into a code address
- `FETCH_HTTPS` and `FETCH_ROUNDS` changing to impossible raw byte values
- nearby bytes such as `AGENT_MODE` also becoming garbage

The fix now landed is simple and concrete:

- increase the guest stack from `16 KiB` to `256 KiB` in
  [runtime/linker.ld](/Users/sangwf/code/MiniAgentOS/runtime/linker.ld)

Validation after that change:

- the first automatic local TLS fetch still succeeds
- a second manual `tls-local` request succeeds
- a third manual `tls-local` request also succeeds
- `fetch-status` during the third run shows intact metadata:
  - `https=true`
  - `path=/`
  - `domain_len=9`
  - `path_len=1`

This does not solve every native-network issue. In the current host
environment, the real OpenAI path can still fail earlier at DNS resolution with
`dns lookup timed out`. But the specific "second native TLS request corrupts
global fetch state" bug is now explained and fixed.

## Current Implementation Map

### Transport layering

- TCP and SOCKS5 are managed in the fetch state machine in
  [runtime/src/main.rs](/Users/sangwf/code/MiniAgentOS/runtime/src/main.rs).
- mbedTLS is wrapped in
  [runtime/mbedtls_wrap.c](/Users/sangwf/code/MiniAgentOS/runtime/mbedtls_wrap.c).
- The guest-side TLS BIO is implemented in
  [runtime/src/tls.rs](/Users/sangwf/code/MiniAgentOS/runtime/src/tls.rs).

### What the current TLS layer does well

- It already uses a separate TLS RX buffer instead of feeding bytes directly
  into the parser.
- It already tracks expected TCP sequence numbers on the TLS ingress path.
- It already hard-resets the mbedTLS heap when the runtime decides the context
  is no longer reusable.

### What is still fragile

#### 1. Security posture is still demo-grade

- Server verification is disabled with `MBEDTLS_SSL_VERIFY_NONE` in
  [runtime/mbedtls_wrap.c](/Users/sangwf/code/MiniAgentOS/runtime/mbedtls_wrap.c#L92).
- No CA chain is configured.
- `time()` always returns `0` in
  [runtime/src/mem.rs](/Users/sangwf/code/MiniAgentOS/runtime/src/mem.rs#L149).
- Entropy is derived from `counter_ticks()` plus an LCG in
  [runtime/src/tls.rs](/Users/sangwf/code/MiniAgentOS/runtime/src/tls.rs#L69).

#### 2. TLS ingress still lacks full TCP overlap handling

The TLS path accepts:

- exact in-order data
- exact future segments buffered in a few pending slots

But it still drops overlapping retransmits rather than trimming them. The key
logic is in
[runtime/src/tls.rs](/Users/sangwf/code/MiniAgentOS/runtime/src/tls.rs#L285).

That means a segment like:

- expected ACK = `1000`
- received seq = `990`
- payload length = `40`

contains `10` duplicate bytes and `30` new bytes, but the current code drops
the whole segment. That can corrupt the effective byte stream seen by TLS.

#### 3. OpenAI reuse is decided too high in the stack

The runtime marks an OpenAI transport reusable when the HTTP parser believes a
complete response body has been received. See
[runtime/src/main.rs](/Users/sangwf/code/MiniAgentOS/runtime/src/main.rs#L3519)
and
[runtime/src/main.rs](/Users/sangwf/code/MiniAgentOS/runtime/src/main.rs#L3595).

That is convenient, but it ties reuse to HTTP completion rather than to a
cleanly managed TLS/socket lifecycle.

#### 4. TLS closure is not explicit

The runtime currently closes failed or non-reusable transports with a plain TCP
FIN path in
[runtime/src/main.rs](/Users/sangwf/code/MiniAgentOS/runtime/src/main.rs#L3546).

It does not currently send `mbedtls_ssl_close_notify()` first.

#### 5. Diagnostics are still too shallow

The runtime traces:

- the main mbedTLS return code
- the last x509 error
- some curve and ServerKeyExchange diagnostics

But it does not expose:

- peer verification result
- fatal alert description
- pending-record state before reuse
- whether a close-notify exchange completed

## Reproduced Failure Signature

Live run from the guest shell with:

```sh
env -u MINIOS_USE_OPENAI_HOST_BRIDGE \
  MINIOS_USE_HOST_SOCKS5_PROXY=1 \
  MINIOS_HOST_SOCKS5_PORT=10808 \
  make run-net-legacy
```

Observed sequence:

1. `hello`
   - succeeds
2. second OpenAI-backed request in the same guest session
   - first fails with `tls read err: 0xffffffffffff8e80`
   - this is `MBEDTLS_ERR_SSL_INVALID_MAC`
3. retry on a fresh connection
   - fails during handshake with `ret=-30592`
   - this is `MBEDTLS_ERR_SSL_FATAL_ALERT_MESSAGE`

Those values correspond to the bundled mbedTLS definitions in
[runtime/third_party/mbedtls/include/mbedtls/ssl.h](/Users/sangwf/code/MiniAgentOS/runtime/third_party/mbedtls/include/mbedtls/ssl.h#L72)
and
[runtime/third_party/mbedtls/include/mbedtls/ssl.h](/Users/sangwf/code/MiniAgentOS/runtime/third_party/mbedtls/include/mbedtls/ssl.h#L96).

## Earlier Deep-Dive Findings

Before the stack-overflow root cause was confirmed, deeper local RSA repro work
narrowed the apparent failure surface significantly.

### What is now ruled out

- RSA `ClientKeyExchange` decryption itself is not failing:
  - decrypting the captured ciphertext from the local OpenSSL server log yields
    a valid 48-byte premaster secret starting with `0x0303`
- TLS PRF derivation is not failing:
  - host-side derivation of the master secret and keyblock matches the guest's
    exported hashes on both handshakes
- Record HMAC construction is not the remaining mismatch:
  - guest-side `tls_mac_diag` reports `mac_match=true` even on the failing
    second handshake

### What is still suspicious

- On the failing second handshake, the guest exports the correct
  `client_write_key_prefix` and `client_write_key_hash`, so the raw negotiated
  client write key bytes are correct.
- But AES-ECB of a zero block under that same derived key diverges from the
  host-side expectation.
- The same wrong AES result appears both:
  - in a fresh local AES context used only for diagnostics
  - in the active TLS `cipher_ctx_enc`

At the time, that made the remaining bug look like it sat below RSA, PRF, and
record-HMAC generation, and closer to outbound encryption or AES context
setup.

With the later stack-overflow finding, these AES-side anomalies should now be
treated cautiously: they were gathered while the guest was still capable of
silently scribbling over adjacent global state.

### ROM-table hardening result

As a defensive hardening step, MiniAgentOS now enables
`MBEDTLS_AES_ROM_TABLES` in
[runtime/mbedtls_config.h](/Users/sangwf/code/MiniAgentOS/runtime/mbedtls_config.h).

Static verification of the rebuilt kernel image with `nm` shows:

- `FSb`
- `FT0`
- `RT0`
- `RSb`

have moved into read-only symbols instead of writable BSS.

However, the local OpenSSL repro still ends the second handshake with
`fatal bad_record_mac`, so writable runtime-generated AES tables were not the
only root cause.

## Short-Term Hardening Checklist

The short-term goal is not "perfect TLS". It is "stop the corruptions and make
the guest-native path debuggable enough to trust test results".

### P0. Keep a TLS-sized guest stack

- Keep the larger stack in
  [runtime/linker.ld](/Users/sangwf/code/MiniAgentOS/runtime/linker.ld).
- Do not shrink it again unless there is a measured stack profile showing a
  lower safe bound.

Rationale:

- The prior `16 KiB` stack was not enough for the native TLS path.
- The resulting overflow corrupted `.bss` and produced misleading TLS
  symptoms.

Acceptance:

- repeated local `tls-local` runs succeed without fetch metadata corruption
- no adjacent agent/fetch globals change to impossible raw values during the
  repro

### P1. Stop risky reuse by default

- Disable guest-native OpenAI transport reuse by default.
- Keep a feature flag to re-enable it for experiments.
- Only reintroduce default reuse after the checklist below passes.

Rationale:

- The current failure signature appears after connection lifecycle complexity,
  not on the simplest one-shot path.

### P1.5. Keep AES tables read-only

- Keep `MBEDTLS_AES_ROM_TABLES` enabled in
  [runtime/mbedtls_config.h](/Users/sangwf/code/MiniAgentOS/runtime/mbedtls_config.h)
  unless there is a measured regression that justifies reverting it.

Rationale:

- It removes one class of writable crypto-global state from the guest image.
- It did not fully fix the second-handshake failure, but it is still a good
  hardening move and reduces the search space for future corruption.

### P2. Fix TLS ingress overlap handling

- Port the overlap-trimming logic mindset from the plain HTTP fix into the TLS
  ingress path.
- Teach `push_rx_payload_seq()` to handle:
  - exact duplicates
  - partial overlaps before `expected_ack`
  - future segments that overlap buffered segments
- Increase pending slot tolerance or switch from "few whole pending segments" to
  a byte-range-aware reassembly policy.

Acceptance:

- repeated retransmit and overlap injection does not change the logical byte
  stream delivered to mbedTLS
- no `INVALID_MAC` under overlap/retransmit stress

### P3. Make TLS closure explicit

- Add a wrapper for `mbedtls_ssl_close_notify()`.
- On graceful close:
  - send close-notify
  - drain until peer close or bounded timeout
  - then send TCP FIN
- On fatal error:
  - immediately mark the context dead
  - do not reuse the connection

Acceptance:

- TLS closure path can distinguish:
  - graceful peer close
  - local close-notify path
  - fatal teardown

### P4. Tighten context reuse rules

- After any read/write/handshake error other than WANT_READ/WANT_WRITE:
  - treat the SSL context as unusable
  - close the underlying connection
  - reset the TLS context before reconnect
- Before any reuse decision:
  - check `mbedtls_ssl_check_pending()`
  - refuse reuse if internal records remain buffered
- Only mark transport reusable if:
  - HTTP response is complete
  - peer did not close
  - TLS has no pending records
  - no transport error occurred during the turn

Acceptance:

- one failed TLS turn cannot poison the next connection attempt

### P4. Improve diagnostics

- Add wrappers and trace fields for:
  - `mbedtls_ssl_get_verify_result()`
  - fatal alert information where available
  - whether `close_notify` was attempted and completed
  - whether `check_pending()` blocked reuse
- Map common negative return codes to short textual reasons in trace.

Acceptance:

- a failed live run should tell us whether the root cause was:
  - record corruption
  - peer alert
  - verification failure
  - premature close
  - timeout

### P5. Add the minimum security prerequisites

- Introduce a real wall-clock source before enabling certificate expiry checks.
- Add a trust model:
  - preferred: CA chain
  - acceptable first step: explicit certificate or SPKI pinning for a narrow
    provider set
- Move from `VERIFY_NONE` to `VERIFY_REQUIRED` once trust material and time are
  in place.
- Replace the timer+LCG entropy source with:
  - hardware RNG if available, or
  - a stronger platform entropy source, or
  - an NV-seed-based bootstrap plus runtime refresh

Acceptance:

- guest-native TLS can reject an intentionally untrusted endpoint
- verification failures are visible in trace

## Medium-Term Refactor Roadmap

### Stage A. Introduce a real stream boundary

Create a guest-side `TcpStream` abstraction that owns:

- connect
- send
- receive
- seq and ACK handling
- close
- connection health

Then make TLS consume that stream abstraction instead of reaching into the
fetch state machine directly.

Why:

- today `main.rs` still owns too much of TCP, HTTP, and TLS lifecycle at once
- this makes retry and reuse correctness hard to reason about

### Stage B. Introduce a `TlsStream` layer

Build a `TlsStream` wrapper above `TcpStream` that owns:

- mbedTLS context lifetime
- BIO callbacks
- handshake
- read and write retry discipline
- close-notify
- reuse policy

Then make HTTP/OpenAI code operate on `TlsStream`, not on raw fetch states.

This is the direction used by stacks like lwIP `altcp_tls`: the application
speaks to a layered TCP abstraction instead of manually embedding TLS details.

### Stage C. Separate credential and policy management

Add a dedicated TLS configuration surface for:

- hostname
- CA or pin set
- verification mode
- minimum TLS version
- ciphersuite policy
- proxy or direct transport choice

This is conceptually closer to Zephyr's secure sockets model, where the socket
and TLS layer own credential and verification setup, not the application logic.

### Stage D. Re-enable optimized reuse only after the new layering holds

After `TcpStream` and `TlsStream` exist:

- reintroduce persistent OpenAI transports behind the new abstractions
- keep reuse policy in the transport layer
- keep HTTP and agent loop logic unaware of the low-level reuse mechanics

## Validation Matrix

The guest-native path should not be considered recovered until all of these
pass.

### Live-path stability

- 20 sequential `hello` requests succeed over guest-native TLS
- 10 two-turn conversational follow-ups succeed
- 5 M5 tool-call follow-up turns succeed without the host OpenAI bridge

### Lifecycle correctness

- repeated connect/close cycles do not produce `INVALID_MAC`
- failed handshakes do not poison subsequent fresh connections
- a peer close-notify is observed and handled correctly

### Security correctness

- valid trusted endpoint succeeds
- intentionally untrusted endpoint fails with verification-visible trace
- wrong hostname fails once hostname verification is enabled

### Transport stress

- duplicate and overlapping TCP payload injection does not corrupt TLS records
- delayed FIN and retransmit cases do not produce silent truncation or MAC
  errors

## Recommended Order

1. Disable reuse by default.
2. Fix TLS overlap handling.
3. Add close-notify and stronger diagnostics.
4. Re-test one-shot native TLS until stable.
5. Re-test native follow-up turns with reuse still disabled.
6. Add time, trust material, and real verification.
7. Only then revisit transport reuse.
8. In parallel, design the `TcpStream` and `TlsStream` split.

## Reference Patterns

- Mbed TLS SSL API:
  [ssl.h](https://mbed-tls.readthedocs.io/projects/api/en/v3.6.5/api/file/ssl_8h/)
- Mbed TLS RNG and entropy guidance:
  [random data generation](https://mbed-tls.readthedocs.io/en/latest/kb/how-to/add-a-random-generator.html)
  and
  [adding entropy sources](https://mbed-tls.readthedocs.io/en/latest/kb/how-to/add-entropy-sources-to-entropy-pool/)
- Mbed TLS porting guidance:
  [porting to a new environment](https://mbed-tls.readthedocs.io/en/latest/kb/how-to/how-do-i-port-mbed-tls-to-a-new-environment-OS/)
- lwIP layered TCP and TLS integration:
  [altcp overview](https://lwip.nongnu.org/2_1_x/group__altcp__api.html)
  and
  [altcp TLS](https://www.nongnu.org/lwip/2_1_x/group__altcp__tls.html)
- Zephyr secure sockets and TLS credentials:
  [secure sockets](https://docs.zephyrproject.org/latest/connectivity/networking/api/sockets.html)
  and
  [TLS socket options](https://docs.zephyrproject.org/latest/doxygen/html/group__secure__sockets__options.html)
