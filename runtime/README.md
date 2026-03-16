# MiniOS Runtime (AArch64 QEMU)

## Build & Run

```bash
make run
```

If `OPENAI_API_KEY` is set in the build shell, MiniOS now embeds that key into
the guest binary at build time and auto-loads it on boot. That makes the M3
natural-language path usable without typing `openai-key ...` on every session.
You can still override it interactively with `openai-key <key>` or clear the
active in-memory key with `openai-clear`.

Expected output:

```
boot ok
```

## Current M4 shell

The default `Goal >` path is now the M4 session loop:

- ordinary natural-language requests go to the guest-direct OpenAI-backed loop
- the loop can call bounded tools such as:
  - `fetch_url`
  - `post_url`
  - `post_tweet`
  - `search_recent_posts`
  - `get_user_posts`
  - `read_session_state`
  - `write_session_state`
- consecutive OpenAI-only turns reuse a guest-side persistent OpenAI transport
  when possible

Typical manual session:

```text
Goal > hello, who are you?
Hello — I’m MiniAgentOS M4, an AI assistant...
Goal > post a tweet: Harness Engineering is interesting.
Tweet posted: "Harness Engineering is interesting." (id: ...)
Goal >
```

Useful shell commands:

- `status inline`
- `status plain`
- `trace on`
- `trace off`
- `debug on`
- `debug off`
- `openai-status`
- `openai-key <key>`
- `openai-clear`
- `session status`
- `session reset`

## UDP Echo Demo

Start QEMU with UDP host forwarding:

```bash
make run-net-legacy-fwd
```

In another terminal, run the interactive client:

```bash
python3 test.py
```

Example inputs:

- `ping` -> `pong`
- `mac` -> `mac=02:00:00:00:00:01`
- `echo hello` -> `hello`
- any other text -> `Hello, I am WalleOS. <text>`

Example session:

```
> hello
recv: b'WalleOS t=7255 c=0 Hello, I am WalleOS. hello' from ('127.0.0.1', 5555)
> ping
recv: b'WalleOS t=9987 c=1 pong' from ('127.0.0.1', 5555)
> mac
recv: b'WalleOS t=13138 c=2 mac=02:00:00:00:00:01' from ('127.0.0.1', 5555)
> echo hello
recv: b'WalleOS t=18598 c=3 hello' from ('127.0.0.1', 5555)
```

## External Network Demo (DNS + HTTP/HTTPS GET)

Run QEMU with user-net (NAT) to allow outbound access:

```bash
make run-net-legacy
```

Expected logs in QEMU:

- `dns ok`
- `tcp connect`
- `tcp send http`
- `http resp:` followed by up to 2 KiB of response bytes (headers + body).

Notes:
- This demo resolves domains via the user-net DNS server (`10.0.2.3`) and sends an HTTP/1.1 GET over TCP (port 80).
- `neverssl.com` is a reliable plain-HTTP target; `example.com` can be flaky over plain HTTP.
- UDP alone does not fetch web pages; HTTP/HTTPS uses TCP (or QUIC/HTTP3, not implemented here).
- HTTPS uses mbedTLS in-kernel (TLS 1.2). Certificate verification is currently disabled.

## URL Fetch via UDP

With `make run-net-legacy-fwd` and `python3 test.py`, you can type a URL and MiniOS will fetch it and print the response on the QEMU console:

- `http://example.com/`
- `https://example.com/`
- `example.com/`
- `get example.com /`
- `post https://httpbin.org/post {"hello":"world"}`
- `sync` (fetch Date header to set OAuth time)
- `time <unix_seconds>` (set OAuth time)
- `tweet <text>` (post to X/Twitter via OAuth 1.0a)
- `post_tweet <text>` (same X/Twitter post flow, aligned with the future M4 tool name)

The UDP reply will say `fetching ...` and `http ok`/`http fail`. Up to 4KB of HTTP headers are sent in `http chunk <idx> ...` messages, and the response body is sent in `body chunk <idx> ...`. If the response is JSON, body chunks use `json chunk <idx> ...`. The QEMU console also prints response bytes.
If a fetch is already pending, MiniOS replies `busy`.

Notes:
- HTTPS supports SOCKS5 proxy at `10.0.2.2:7897`; used for HTTPS only.
- Redirects (301/302) are followed automatically up to 3 hops.

## X/Twitter integration

MiniOS can post Tweets via OAuth 1.0a (`POST https://api.twitter.com/2/tweets`) and
can also read X/Twitter data through bearer-token-backed endpoints.
Secrets are loaded at build time from either `xsecret.txt` (ignored by git) or
shell environment variables.

Example `xsecret.txt`:

```
API Key: ...
API Secret: ...
Access Token: ...
Access Token Secret: ...
```

Equivalent shell variables:

```sh
export X_CONSUMER_KEY=...
export X_CONSUMER_KEY_SECRET=...
export X_ACCESS_TOKEN=...
export X_ACCESS_TOKEN_SECRET=...
```

For X read APIs, also set:

```sh
export X_BEARER_TOKEN=...
```

Current mapping:

- `post_tweet` uses:
  - `X_CONSUMER_KEY`
  - `X_CONSUMER_KEY_SECRET`
  - `X_ACCESS_TOKEN`
  - `X_ACCESS_TOKEN_SECRET`
- `search_recent_posts` and `get_user_posts` use:
  - `X_BEARER_TOKEN`

Usage:

1) Start QEMU with UDP forwarding:
   `make run-net-legacy-fwd`
2) Set time (required for OAuth timestamp):
   `time <unix_seconds>`
3) Post a tweet:
   `tweet hello from minios`
   or
   `post_tweet hello from minios`

Expected response: HTTP `201 Created` with a JSON body containing the new tweet id.

For read-style prompts in the M4 shell, examples include:

- `Andrej Karpathy's latest opinions in twitter.`
- `Search recent posts about ai agents.`

## Dependencies

Open-source components:
- mbedTLS (vendored under `third_party/mbedtls`)
- `cc` crate for building C sources
- QEMU (runtime environment)

In-house components:
- Boot/exception handling, MMIO, virtio-net driver
- Minimal network stack (ARP/DNS/UDP/TCP/ICMP/HTTP)
- TLS adapter glue (`src/tls.rs`, `mbedtls_wrap.c`)
- UDP control plane + fetch/redirect/chunking logic
- OAuth 1.0a signing (HMAC-SHA1 + base64 via mbedTLS)

## QEMU Console Output

When issuing fetches directly in the QEMU console (UART), MiniOS prints:
- `--- headers ---` followed by headers (up to 4KB)
- `--- body ---` or `--- json ---` for response body (up to 4KB)
- a single `http ok|fail <domain>` summary line

## UART Console Fetch (Recommended)

With `make run-net-legacy-fwd`, you can type URLs directly in the QEMU console:

```
> http://neverssl.com/
```

The console will show fetch progress logs and finish with `http ok <domain>` or `http fail <domain>`.
`AUTO_FETCH` is disabled, so commands only run when you type them.

## Notes
- For DMA/MMIO buffers, avoid `core::ptr::read_volatile`. Use inline-asm helpers in `mmio` (e.g., `read8/read16/read32`) to prevent Rust UB precondition checks from panicking in kernel.
