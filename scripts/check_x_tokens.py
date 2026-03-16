#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import hashlib
import hmac
import json
import os
import secrets
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass


API_BASE = "https://api.x.com"
DEFAULT_USERNAME = "sangwf2001"
USER_AGENT = "MiniAgentOS-X-Token-Check/1.0"


@dataclass
class HttpResult:
    ok: bool
    status: int
    reason: str
    body: str


def shell_env_value(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if value:
        return value
    for shell in ("zsh", "bash"):
        try:
            proc = subprocess.run(
                [shell, "-ic", f'printf %s "${{{name}:-}}"'],
                capture_output=True,
                text=True,
                timeout=5,
                check=False,
            )
        except (FileNotFoundError, subprocess.TimeoutExpired):
            continue
        candidate = proc.stdout.strip()
        if candidate:
            return candidate
    return ""


def request(
    method: str,
    url: str,
    headers: dict[str, str] | None = None,
    body: bytes | None = None,
) -> HttpResult:
    req = urllib.request.Request(
        url,
        data=body,
        headers={"User-Agent": USER_AGENT, **(headers or {})},
        method=method,
    )
    try:
        with urllib.request.urlopen(req, timeout=20) as resp:
            payload = resp.read().decode("utf-8", errors="replace")
            return HttpResult(True, resp.status, "ok", payload)
    except urllib.error.HTTPError as exc:
        payload = exc.read().decode("utf-8", errors="replace")
        return HttpResult(False, exc.code, exc.reason or "http error", payload)
    except urllib.error.URLError as exc:
        return HttpResult(False, 0, str(exc.reason), "")


def oauth_percent_encode(value: str) -> str:
    return urllib.parse.quote(value, safe="~-._")


def build_oauth1_header(
    method: str,
    url: str,
    consumer_key: str,
    consumer_secret: str,
    access_token: str,
    access_token_secret: str,
) -> str:
    parsed = urllib.parse.urlsplit(url)
    base_url = f"{parsed.scheme}://{parsed.netloc}{parsed.path}"
    query_pairs = urllib.parse.parse_qsl(parsed.query, keep_blank_values=True)

    oauth_params = {
        "oauth_consumer_key": consumer_key,
        "oauth_nonce": secrets.token_hex(16),
        "oauth_signature_method": "HMAC-SHA1",
        "oauth_timestamp": str(int(time.time())),
        "oauth_token": access_token,
        "oauth_version": "1.0",
    }

    signature_pairs = query_pairs + list(oauth_params.items())
    signature_pairs.sort(key=lambda item: (item[0], item[1]))
    normalized = "&".join(
        f"{oauth_percent_encode(k)}={oauth_percent_encode(v)}"
        for k, v in signature_pairs
    )
    signature_base = "&".join(
        [
            method.upper(),
            oauth_percent_encode(base_url),
            oauth_percent_encode(normalized),
        ]
    )
    signing_key = (
        f"{oauth_percent_encode(consumer_secret)}&"
        f"{oauth_percent_encode(access_token_secret)}"
    )
    digest = hmac.new(
        signing_key.encode("utf-8"),
        signature_base.encode("utf-8"),
        hashlib.sha1,
    ).digest()
    oauth_params["oauth_signature"] = base64.b64encode(digest).decode("ascii")

    header_items = ", ".join(
        f'{oauth_percent_encode(k)}="{oauth_percent_encode(v)}"'
        for k, v in sorted(oauth_params.items())
    )
    return "OAuth " + header_items


def print_result(label: str, result: HttpResult) -> None:
    status_text = f"{result.status}" if result.status else "n/a"
    state = "PASS" if result.ok else "FAIL"
    print(f"[{state}] {label}: status={status_text} reason={result.reason}")
    if result.body:
        snippet = result.body.strip()
        if len(snippet) > 400:
            snippet = snippet[:400] + "...(truncated)"
        print(snippet)
    print()


def check_bearer_token(username: str, bearer_token: str) -> HttpResult:
    if not bearer_token:
        return HttpResult(False, 0, "missing X_BEARER_TOKEN", "")
    url = f"{API_BASE}/2/users/by/username/{urllib.parse.quote(username)}"
    headers = {"Authorization": f"Bearer {bearer_token}"}
    return request("GET", url, headers=headers)


def check_oauth1_user_context(
    consumer_key: str,
    consumer_secret: str,
    access_token: str,
    access_token_secret: str,
) -> HttpResult:
    missing = [
        name
        for name, value in (
            ("X_CONSUMER_KEY", consumer_key),
            ("X_CONSUMER_KEY_SECRET", consumer_secret),
            ("X_ACCESS_TOKEN", access_token),
            ("X_ACCESS_TOKEN_SECRET", access_token_secret),
        )
        if not value
    ]
    if missing:
        return HttpResult(False, 0, "missing " + ", ".join(missing), "")
    url = f"{API_BASE}/2/users/me"
    headers = {
        "Authorization": build_oauth1_header(
            "GET",
            url,
            consumer_key,
            consumer_secret,
            access_token,
            access_token_secret,
        )
    }
    return request("GET", url, headers=headers)


def check_oauth1_write(
    consumer_key: str,
    consumer_secret: str,
    access_token: str,
    access_token_secret: str,
    text: str,
) -> HttpResult:
    url = f"{API_BASE}/2/tweets"
    payload = json.dumps({"text": text}, ensure_ascii=False).encode("utf-8")
    headers = {
        "Authorization": build_oauth1_header(
            "POST",
            url,
            consumer_key,
            consumer_secret,
            access_token,
            access_token_secret,
        ),
        "Content-Type": "application/json",
    }
    return request("POST", url, headers=headers, body=payload)


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Check X credentials used by MiniAgentOS. "
            "Default checks are read-only; use --post-text to do a real write test."
        )
    )
    parser.add_argument(
        "--username",
        default=DEFAULT_USERNAME,
        help="Username used for the bearer-token public lookup check",
    )
    parser.add_argument(
        "--post-text",
        help="If set, perform a real POST /2/tweets write check with this text",
    )
    args = parser.parse_args()

    bearer_token = shell_env_value("X_BEARER_TOKEN")
    consumer_key = shell_env_value("X_CONSUMER_KEY")
    consumer_secret = shell_env_value("X_CONSUMER_KEY_SECRET")
    access_token = shell_env_value("X_ACCESS_TOKEN") or shell_env_value("X_ACCESS_TOEKN")
    access_token_secret = shell_env_value("X_ACCESS_TOKEN_SECRET")

    print("MiniAgentOS X token check")
    print(f"- username check: {args.username}")
    print(f"- bearer token present: {'yes' if bearer_token else 'no'}")
    print(
        "- oauth1 secrets present: "
        + (
            "yes"
            if all([consumer_key, consumer_secret, access_token, access_token_secret])
            else "no"
        )
    )
    print()

    bearer_result = check_bearer_token(args.username, bearer_token)
    print_result("Bearer token public user lookup", bearer_result)

    oauth1_result = check_oauth1_user_context(
        consumer_key,
        consumer_secret,
        access_token,
        access_token_secret,
    )
    print_result("OAuth 1.0a user-context auth (/2/users/me)", oauth1_result)

    if args.post_text:
        write_result = check_oauth1_write(
            consumer_key,
            consumer_secret,
            access_token,
            access_token_secret,
            args.post_text,
        )
        print_result("OAuth 1.0a write check (POST /2/tweets)", write_result)

    failed = not bearer_result.ok or not oauth1_result.ok
    if args.post_text:
        failed = failed or (not write_result.ok)
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
