from __future__ import annotations

import json
import os
import re
import subprocess
import threading
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse


@dataclass
class CapturedRequest:
    path: str
    body: bytes
    headers: dict[str, str]


@dataclass
class CapturedRequestState:
    requests: list[CapturedRequest] = field(default_factory=list)


@dataclass
class XFixtureState:
    post_requests: list[CapturedRequest] = field(default_factory=list)
    search_requests: list[CapturedRequest] = field(default_factory=list)
    user_posts_requests: list[CapturedRequest] = field(default_factory=list)


class ManagedHTTPServer:
    def __init__(self, server: ThreadingHTTPServer):
        self.server = server
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def start(self):
        self.thread.start()

    def stop(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)


class ReusableThreadingHTTPServer(ThreadingHTTPServer):
    allow_reuse_address = True
    daemon_threads = True


def start_result_sink(bind_host: str, port: int, path: str):
    state = CapturedRequestState()

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            content_length = int(self.headers.get("Content-Length", "0"))
            body = self.rfile.read(content_length)
            state.requests.append(
                CapturedRequest(
                    path=self.path,
                    body=body,
                    headers={key: value for key, value in self.headers.items()},
                )
            )
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(b'{"ok":true}\n')

        def log_message(self, format, *args):
            return

    server = ReusableThreadingHTTPServer((bind_host, port), Handler)
    managed = ManagedHTTPServer(server)
    managed.start()
    return managed, state, f"http://{bind_host}:{server.server_port}{path}"


def start_source_fixture(
    bind_host: str,
    port: int,
    path: str,
    content: str,
    pages: dict[str, str] | None = None,
):
    pages = pages or {}
    payloads = {path: content.encode("utf-8")}
    for extra_path, extra_content in pages.items():
        payloads[str(extra_path)] = str(extra_content).encode("utf-8")
    state = CapturedRequestState()

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            payload = payloads.get(self.path)
            if payload is None:
                self.send_response(404)
                self.end_headers()
                return
            state.requests.append(
                CapturedRequest(
                    path=self.path,
                    body=b"",
                    headers={key: value for key, value in self.headers.items()},
                )
            )
            self.send_response(200)
            self.send_header("Content-Type", "text/markdown; charset=utf-8")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def log_message(self, format, *args):
            return

    server = ReusableThreadingHTTPServer((bind_host, port), Handler)
    managed = ManagedHTTPServer(server)
    managed.start()
    return managed, state, f"http://{bind_host}:{server.server_port}{path}"


def _gateway_summary(text: str, max_items: int) -> str:
    normalized = " ".join(text.replace("\n", " ").split())
    if not normalized:
        return ""
    max_items = max(1, min(max_items, 6))
    chunks = []
    for part in normalized.split("."):
        part = part.strip()
        if not part:
            continue
        chunks.append("- " + part.rstrip("."))
        if len(chunks) >= max_items:
            break
    return "\n".join(chunks)


def _shell_env_value(name: str) -> str | None:
    value = os.environ.get(name)
    if value:
        return value
    for shell in ("zsh", "bash"):
        try:
            proc = subprocess.run(
                [shell, "-ic", f'printf %s "${{{name}:-}}"'],
                capture_output=True,
                text=True,
                timeout=5,
            )
        except (FileNotFoundError, subprocess.TimeoutExpired):
            continue
        if proc.returncode == 0 and proc.stdout:
            return proc.stdout
    return None


def _extract_output_text(payload: dict) -> str:
    text = payload.get("output_text")
    if isinstance(text, str) and text.strip():
        return text.strip()
    output = payload.get("output")
    if not isinstance(output, list):
        return ""
    parts: list[str] = []
    for item in output:
        if not isinstance(item, dict):
            continue
        content = item.get("content")
        if not isinstance(content, list):
            continue
        for block in content:
            if not isinstance(block, dict):
                continue
            text = block.get("text")
            if isinstance(text, str) and text:
                parts.append(text)
    return "\n".join(parts).strip()


def _strip_json_fences(text: str) -> str:
    text = text.strip()
    if text.startswith("```"):
        lines = text.splitlines()
        if lines and lines[0].startswith("```"):
            lines = lines[1:]
        if lines and lines[-1].strip() == "```":
            lines = lines[:-1]
        return "\n".join(lines).strip()
    return text


def _detect_goal_max_items(goal_text: str) -> int:
    lowered = goal_text.lower()
    match = re.search(r"\b(\d+)\s+(?:bullet|bullets|point|points|item|items)\b", lowered)
    if match:
        return max(1, min(int(match.group(1)), 8))
    word_map = {
        "one": 1,
        "two": 2,
        "three": 3,
        "four": 4,
        "five": 5,
        "six": 6,
    }
    for word, value in word_map.items():
        if re.search(rf"\b{word}\s+(?:bullet|bullets|point|points|item|items)\b", lowered):
            return value
    return 3


def _extract_goal_urls(goal_text: str) -> list[str]:
    urls = re.findall(r"https?://\S+", goal_text)
    return [url.rstrip(".,;)") for url in urls]


def _openai_summary(
    source_text: str,
    max_items: int,
    model: str,
    api_key_env: str,
) -> tuple[bool, dict]:
    api_key = _shell_env_value(api_key_env)
    if not api_key:
        return False, {
            "status": "error",
            "reason": f"missing {api_key_env}",
        }

    max_items = max(1, min(max_items, 6))
    instructions = (
        "You are the model gateway for MiniAgentOS. "
        "Summarize the provided source into at most "
        f"{max_items} concise bullet points. "
        "Return only the bullet list as plain text."
    )
    payload = {
        "model": model,
        "instructions": instructions,
        "input": source_text,
        "reasoning": {"effort": "minimal"},
        "max_output_tokens": 220,
    }
    body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    request = urllib.request.Request(
        "https://api.openai.com/v1/responses",
        data=body,
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=45) as response:
            response_body = response.read()
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        return False, {
            "status": "error",
            "reason": f"openai http {exc.code}",
            "detail": detail[:400],
        }
    except urllib.error.URLError as exc:
        return False, {
            "status": "error",
            "reason": f"openai network error: {exc.reason}",
        }

    try:
        response_json = json.loads(response_body.decode("utf-8"))
    except json.JSONDecodeError:
        return False, {
            "status": "error",
            "reason": "invalid openai response json",
        }

    summary = _extract_output_text(response_json)
    if not summary:
        return False, {
            "status": "error",
            "reason": "empty openai response text",
        }
    return True, {
        "status": "ok",
        "summary": summary,
    }


def _mock_interpret_goal(goal_text: str) -> dict:
    lowered = " ".join(goal_text.lower().split())
    if "translation error" in lowered or "broken translator" in lowered:
        return {
            "status": "error",
            "reason": "interpretation backend unavailable",
        }
    has_summary = (
        "summarize" in lowered
        or "summary" in lowered
        or "takeaway" in lowered
        or "takeaways" in lowered
        or "key point" in lowered
        or "key points" in lowered
        or "bullet point" in lowered
        or "bullet points" in lowered
    )
    has_post = "post" in lowered
    urls = _extract_goal_urls(goal_text)
    if not has_summary or not urls:
        return {
            "status": "error",
            "reason": "unsupported goal",
        }
    if has_post:
        if len(urls) < 2:
            return {
                "status": "error",
                "reason": "unsupported goal",
            }
        return {
            "status": "ok",
            "action": "post_summary",
            "source_url": urls[0],
            "sink_url": urls[-1],
            "max_items": _detect_goal_max_items(goal_text),
        }
    return {
        "status": "ok",
        "action": "local_summary",
        "source_url": urls[0],
        "max_items": _detect_goal_max_items(goal_text),
    }


def _openai_interpret_goal(
    goal_text: str,
    model: str,
    api_key_env: str,
) -> tuple[bool, dict]:
    api_key = _shell_env_value(api_key_env)
    if not api_key:
        return False, {
            "status": "error",
            "reason": f"missing {api_key_env}",
        }

    instructions = (
        "You are the goal interpretation gateway for MiniAgentOS. "
        "Supported goals are limited to two families: "
        "1) summarize one source URL directly for the user, "
        "2) summarize one source URL and post the result to one sink URL. "
        "Treat requests like 'read this URL and give me three bullet point takeaways' "
        "as direct summary goals. "
        "Treat 'post the result to <url>' as a posted summary goal. "
        "Return only compact JSON with no prose or markdown fences. "
        "If the user wants a direct summary, return: "
        '{"status":"ok","action":"local_summary","source_url":"...","max_items":3}. '
        "If the user wants a posted summary, return: "
        '{"status":"ok","action":"post_summary","source_url":"...","sink_url":"...","max_items":3}. '
        "Examples: "
        'Input: "Summarize https://example.com in three bullet points." '
        'Output: {"status":"ok","action":"local_summary","source_url":"https://example.com","max_items":3}. '
        'Input: "Please read https://example.com and give me three bullet point takeaways." '
        'Output: {"status":"ok","action":"local_summary","source_url":"https://example.com","max_items":3}. '
        'Input: "Read https://example.com and post a summary to http://10.0.2.2:8081/result." '
        'Output: {"status":"ok","action":"post_summary","source_url":"https://example.com","sink_url":"http://10.0.2.2:8081/result","max_items":3}. '
        'If unsupported, return: {"status":"error","reason":"unsupported goal"}.'
    )
    payload = {
        "model": model,
        "instructions": instructions,
        "input": goal_text,
        "reasoning": {"effort": "minimal"},
        "max_output_tokens": 240,
    }
    body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    request = urllib.request.Request(
        "https://api.openai.com/v1/responses",
        data=body,
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=45) as response:
            response_body = response.read()
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        return False, {
            "status": "error",
            "reason": f"openai http {exc.code}",
            "detail": detail[:400],
        }
    except urllib.error.URLError as exc:
        return False, {
            "status": "error",
            "reason": f"openai network error: {exc.reason}",
        }

    try:
        response_json = json.loads(response_body.decode("utf-8"))
    except json.JSONDecodeError:
        return False, {
            "status": "error",
            "reason": "invalid openai response json",
        }

    output_text = _strip_json_fences(_extract_output_text(response_json))
    if not output_text:
        return False, {
            "status": "error",
            "reason": "empty openai response text",
        }
    try:
        translated = json.loads(output_text)
    except json.JSONDecodeError:
        fallback = _mock_interpret_goal(goal_text)
        if fallback.get("status") == "ok":
            return True, fallback
        return False, {
            "status": "error",
            "reason": "invalid translated goal json",
            "detail": output_text[:400],
        }
    if not isinstance(translated, dict):
        fallback = _mock_interpret_goal(goal_text)
        if fallback.get("status") == "ok":
            return True, fallback
        return False, {
            "status": "error",
            "reason": "translated goal was not an object",
        }
    if translated.get("status") == "error" and translated.get("reason") == "unsupported goal":
        fallback = _mock_interpret_goal(goal_text)
        if fallback.get("status") == "ok":
            return True, fallback
    return True, translated


def start_model_gateway(
    bind_host: str,
    port: int,
    ok_path: str,
    error_path: str,
    backend: str = "mock",
    model: str = "gpt-5.4-mini",
    api_key_env: str = "OPENAI_API_KEY",
):
    state = CapturedRequestState()

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            content_length = int(self.headers.get("Content-Length", "0"))
            body = self.rfile.read(content_length)
            state.requests.append(
                CapturedRequest(
                    path=self.path,
                    body=body,
                    headers={key: value for key, value in self.headers.items()},
                )
            )

            if self.path == error_path:
                payload = {
                    "status": "error",
                    "reason": "model backend unavailable",
                }
                encoded = json.dumps(payload, ensure_ascii=True).encode("utf-8")
                self.send_response(502)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(encoded)))
                self.end_headers()
                self.wfile.write(encoded)
                return

            if self.path != ok_path:
                self.send_response(404)
                self.end_headers()
                return

            try:
                request = json.loads(body.decode("utf-8"))
            except json.JSONDecodeError:
                self.send_response(400)
                self.end_headers()
                return

            if backend == "openai":
                ok, payload = _openai_summary(
                    str(request.get("source_text", "")),
                    int(request.get("max_items", 3)),
                    model,
                    api_key_env,
                )
                code = 200 if ok else 502
            else:
                summary = _gateway_summary(
                    str(request.get("source_text", "")),
                    int(request.get("max_items", 3)),
                )
                payload = {
                    "status": "ok",
                    "summary": summary,
                }
                code = 200
            encoded = json.dumps(payload, ensure_ascii=True).encode("utf-8")
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(encoded)))
            self.end_headers()
            self.wfile.write(encoded)

        def log_message(self, format, *args):
            return

    server = ReusableThreadingHTTPServer((bind_host, port), Handler)
    managed = ManagedHTTPServer(server)
    managed.start()
    return managed, state, (
        f"http://{bind_host}:{server.server_port}{ok_path}",
        f"http://{bind_host}:{server.server_port}{error_path}",
    )


def start_interpretation_gateway(
    bind_host: str,
    port: int,
    ok_path: str,
    error_path: str,
    backend: str = "mock",
    model: str = "gpt-5.4-mini",
    api_key_env: str = "OPENAI_API_KEY",
):
    state = CapturedRequestState()

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            content_length = int(self.headers.get("Content-Length", "0"))
            body = self.rfile.read(content_length)
            state.requests.append(
                CapturedRequest(
                    path=self.path,
                    body=body,
                    headers={key: value for key, value in self.headers.items()},
                )
            )

            if self.path == error_path:
                payload = {
                    "status": "error",
                    "reason": "interpretation backend unavailable",
                }
                encoded = json.dumps(payload, ensure_ascii=True).encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(encoded)))
                self.end_headers()
                self.wfile.write(encoded)
                return

            if self.path != ok_path:
                self.send_response(404)
                self.end_headers()
                return

            try:
                request = json.loads(body.decode("utf-8"))
            except json.JSONDecodeError:
                self.send_response(400)
                self.end_headers()
                return

            goal_text = str(request.get("goal_text", ""))
            if backend == "openai":
                ok, payload = _openai_interpret_goal(goal_text, model, api_key_env)
                code = 200 if ok else 502
            else:
                payload = _mock_interpret_goal(goal_text)
                code = 200
            encoded = json.dumps(payload, ensure_ascii=True).encode("utf-8")
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(encoded)))
            self.end_headers()
            self.wfile.write(encoded)

        def log_message(self, format, *args):
            return

    server = ReusableThreadingHTTPServer((bind_host, port), Handler)
    managed = ManagedHTTPServer(server)
    managed.start()
    return managed, state, (
        f"http://{bind_host}:{server.server_port}{ok_path}",
        f"http://{bind_host}:{server.server_port}{error_path}",
    )


def start_translation_gateway(
    bind_host: str,
    port: int,
    ok_path: str,
    error_path: str,
    backend: str = "mock",
    model: str = "gpt-5.4-mini",
    api_key_env: str = "OPENAI_API_KEY",
):
    return start_interpretation_gateway(
        bind_host,
        port,
        ok_path,
        error_path,
        backend=backend,
        model=model,
        api_key_env=api_key_env,
    )


def start_x_fixture(
    bind_host: str,
    port: int,
    post_path: str,
    search_path: str,
    user_posts_path: str,
    fixture_data: dict | None = None,
):
    state = XFixtureState()
    fixture_data = fixture_data or {}
    default_post_result = fixture_data.get(
        "post_result",
        {
            "status": "ok",
            "tweet_id": "tweet-001",
        },
    )
    search_index = fixture_data.get("search", {})
    user_index = fixture_data.get("users", {})

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            content_length = int(self.headers.get("Content-Length", "0"))
            body = self.rfile.read(content_length)
            if self.path != post_path:
                self.send_response(404)
                self.end_headers()
                return
            state.post_requests.append(
                CapturedRequest(
                    path=self.path,
                    body=body,
                    headers={key: value for key, value in self.headers.items()},
                )
            )
            try:
                request = json.loads(body.decode("utf-8"))
            except json.JSONDecodeError:
                request = {}
            payload = dict(default_post_result)
            payload.setdefault("status", "ok")
            if isinstance(request, dict) and "text" in request:
                payload.setdefault("text", request["text"])
            encoded = json.dumps(payload, ensure_ascii=True).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(encoded)))
            self.end_headers()
            self.wfile.write(encoded)

        def do_GET(self):
            parsed = urlparse(self.path)
            if parsed.path == search_path:
                state.search_requests.append(
                    CapturedRequest(
                        path=self.path,
                        body=b"",
                        headers={key: value for key, value in self.headers.items()},
                    )
                )
                query = parse_qs(parsed.query).get("query", [""])[0]
                posts = search_index.get(query, search_index.get("*", []))
                payload = {
                    "status": "ok",
                    "query": query,
                    "posts": posts,
                }
            elif parsed.path.startswith(user_posts_path.rstrip("/") + "/"):
                state.user_posts_requests.append(
                    CapturedRequest(
                        path=self.path,
                        body=b"",
                        headers={key: value for key, value in self.headers.items()},
                    )
                )
                username = parsed.path.rsplit("/", 1)[-1]
                payload = {
                    "status": "ok",
                    "username": username,
                    "posts": user_index.get(username, user_index.get("*", [])),
                }
            else:
                self.send_response(404)
                self.end_headers()
                return
            encoded = json.dumps(payload, ensure_ascii=True).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(encoded)))
            self.end_headers()
            self.wfile.write(encoded)

        def log_message(self, format, *args):
            return

    server = ReusableThreadingHTTPServer((bind_host, port), Handler)
    managed = ManagedHTTPServer(server)
    managed.start()
    return managed, state, {
        "post_url": f"http://{bind_host}:{server.server_port}{post_path}",
        "search_url": f"http://{bind_host}:{server.server_port}{search_path}",
        "user_posts_url": f"http://{bind_host}:{server.server_port}{user_posts_path}",
    }


def decode_result_payload(captured: CapturedRequest):
    return json.loads(captured.body.decode("utf-8"))
