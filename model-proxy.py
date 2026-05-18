"""Tiny proxy that rewrites OpenAI model names to provider-specific ones.

Usage:
    python model-proxy.py --provider deepseek
    python model-proxy.py --provider kimi
    python model-proxy.py --provider qwen
    python model-proxy.py --provider siliconflow
    python model-proxy.py --provider zhipu
"""

from http.server import HTTPServer, BaseHTTPRequestHandler
import urllib.request
import urllib.error
import json
import sys
import argparse

RELAY_HOST = "http://127.0.0.1:4446"
LISTEN_PORT = 4447

PROVIDER_MODEL_MAPS = {
    "deepseek": {
        "gpt-5.5": "deepseek-v4-pro",
        "gpt-5.4": "deepseek-v4-flash",
        "gpt-5.4-mini": "deepseek-v4-flash",
        "gpt-5": "deepseek-v4-pro",
    },
    "kimi": {
        "gpt-5.5": "kimi-k2.6",
        "gpt-5.4": "kimi-k2.5",
        "gpt-5.4-mini": "kimi-k2.5",
        "gpt-5": "kimi-k2.6",
    },
    "qwen": {
        "gpt-5.5": "qwen3-max",
        "gpt-5.4": "qwen3.5-plus",
        "gpt-5.4-mini": "qwen3.5-flash",
        "gpt-5": "qwen3-max",
    },
    "siliconflow": {
        "gpt-5.5": "deepseek-ai/DeepSeek-V3",
        "gpt-5.4": "Qwen/Qwen3-235B-A22B",
        "gpt-5.4-mini": "Qwen/Qwen3-235B-A22B",
        "gpt-5": "deepseek-ai/DeepSeek-V3",
    },
    "zhipu": {
        "gpt-5.5": "glm-5.1",
        "gpt-5.4": "glm-4-flash",
        "gpt-5.4-mini": "glm-4-flash",
        "gpt-5": "glm-5.1",
    },
}


class ProxyHandler(BaseHTTPRequestHandler):
    provider = "deepseek"

    def rewrite_model(self, body):
        model_map = PROVIDER_MODEL_MAPS.get(self.provider, {})
        if not model_map:
            return body
        try:
            data = json.loads(body)
            model = data.get("model", "")
            if model in model_map:
                new_model = model_map[model]
                print(f"[model-map:{self.provider}] {model} -> {new_model}")
                data["model"] = new_model
                return json.dumps(data).encode("utf-8")
        except json.JSONDecodeError:
            pass
        return body

    def do_POST(self):
        body = None
        content_length = int(self.headers.get("Content-Length", 0))
        if content_length > 0:
            body = self.rfile.read(content_length)

        if body and self.path in ("/v1/responses", "/v1/chat/completions"):
            body = self.rewrite_model(body)

        target_url = f"{RELAY_HOST}{self.path}"
        req = urllib.request.Request(target_url, data=body, method="POST")

        forward_headers = [
            "Content-Type",
            "Authorization",
            "Accept",
            "User-Agent",
            "X-Request-Id",
            "X-Api-Key",
        ]
        for h in forward_headers:
            v = self.headers.get(h)
            if v:
                req.add_header(h, v)

        try:
            resp = urllib.request.urlopen(req, timeout=120)
            self.send_response(resp.status)
            self.send_header(
                "Content-Type", resp.headers.get("Content-Type", "application/json")
            )
            self.end_headers()
            self.wfile.write(resp.read())
        except urllib.error.HTTPError as e:
            self.send_response(e.code)
            self.end_headers()
            self.wfile.write(e.read())

    def do_GET(self):
        target_url = f"{RELAY_HOST}{self.path}"
        req = urllib.request.Request(target_url, method="GET")
        try:
            resp = urllib.request.urlopen(req, timeout=30)
            self.send_response(resp.status)
            self.send_header(
                "Content-Type", resp.headers.get("Content-Type", "application/json")
            )
            self.end_headers()
            self.wfile.write(resp.read())
        except urllib.error.HTTPError as e:
            self.send_response(e.code)
            self.end_headers()
            self.wfile.write(e.read())


def main():
    parser = argparse.ArgumentParser(
        description="Model name proxy for codex-relay multi-provider support"
    )
    parser.add_argument(
        "--provider",
        default="deepseek",
        choices=list(PROVIDER_MODEL_MAPS.keys()),
        help="Which provider model map to use (default: deepseek)",
    )
    args = parser.parse_args()

    ProxyHandler.provider = args.provider

    print(f"[model-proxy] Provider: {args.provider}")
    print(f"[model-proxy] Model map: {PROVIDER_MODEL_MAPS[args.provider]}")
    print(f"[model-proxy] Listening on http://127.0.0.1:{LISTEN_PORT}")
    print(f"[model-proxy] Forwarding to {RELAY_HOST}")

    server = HTTPServer(("127.0.0.1", LISTEN_PORT), ProxyHandler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n[model-proxy] Shutting down")
        server.shutdown()


if __name__ == "__main__":
    main()
