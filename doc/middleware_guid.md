# 🧭 Request middleware and observability

Experiment 011 adds four tools around your route handlers: request IDs,
structured request logs, browser CORS policies and request timeouts.
They are configured in `settings.rs`; your route code stays small.

## 🧠 Understand the request pipeline

```text
Client request
  → request ID + tracing context
  → browser CORS policy
  → request timeout
  → existing services, routing and handler
  → response with the same request ID + completion log
```

| Tool | Purpose | Framework default |
| --- | --- | --- |
| Request ID | Find the same request in headers and logs. | Enabled |
| Request logging | Record method, route pattern, status and time to response headers. | Disabled |
| CORS | Let an explicitly allowed browser frontend read API responses. | Disabled |
| Timeout | Cancel a pending handler after its configured deadline. | Disabled |

The comprehensive example deliberately enables logging, a two-second timeout
and the frontend origin `http://localhost:3000`.

## 🚀 Start with environment settings

In a generated application's `.env`:

```dotenv
RUVORAQ_REQUEST_ID=true
RUVORAQ_REQUEST_LOG=true
RUVORAQ_REQUEST_TIMEOUT_MS=5000
```

Restart `ruvoraq dev`, then:

```bash
curl -i http://127.0.0.1:8000/ -H 'x-request-id: demo-123'
```

The response includes `x-request-id: demo-123`. The bootstrapped binary installs
JSON logging on stderr when request logging is enabled, preserving an existing
application tracing subscriber. No logging subscriber is installed when logging
is disabled.

Process environment overrides `.env`. These settings are loaded before the
configure hook; explicit builder calls made afterward can override them.
Booleans must be `true` or `false`. Timeout milliseconds must be a positive u64;
zero and invalid values fail before binding, with redacted diagnostics.

## 🔎 Request IDs

A valid incoming ID is preserved. It must contain 1–64 ASCII letters/digits,
hyphens, underscores or dots. Missing, invalid or duplicate ID headers are
replaced with a UUID v4. The canonical ID is passed to handlers and returned on
success, errors, documentation responses, timeouts and genuine preflights.
Handler-written ID headers are replaced so the response matches the request.

```rust
use ruvoraq::prelude::*;

#[get("/request-info")]
async fn request_info(id: RequestId) -> Value {
    json!({"request_id": id.as_str()})
}
```

An ID is correlation data, not authentication or a trusted identity. Avoid
putting secrets in IDs. `RUVORAQ_REQUEST_ID=false` disables automatic handling;
do not use the RequestId extractor when handling is disabled, because its
missing extension produces an internal error.

## 📋 Structured request logs

Completion events use the `ruvoraq::http` tracing target and fields:

```json
{
  "fields": {
    "message": "request completed",
    "request_id": "demo-123",
    "method": "GET",
    "route": "/students/{id}",
    "status": 200,
    "duration_ms": 3
  },
  "target": "ruvoraq::http"
}
```

Logs use the static route pattern, not a URL parameter value. Unmatched paths
are recorded as `<unmatched>`. Request-completion events omit raw query strings,
request bodies, authorization headers and cookies. Existing application logs and
panic hooks have their own behavior; this policy applies to Ruvoraq's request events.

Tracing spans also carry the ID, method and route while the handler future runs.
For manual App builders or router integrations, call `ruvoraq::init_logging()`
at your binary entry point or install your own tracing subscriber. The bootstrap
macro does this automatically when logging is enabled.

## 🌐 CORS: connect a browser frontend

CORS controls which browser origins can read responses. It is not an
authentication mechanism and does not block ordinary curl/server clients.

Replace the existing bootstrap invocation in `settings.rs` with the following
hook (extend your existing hook if one is already present):

```rust
fn configure(app: ruvoraq::App) -> std::io::Result<ruvoraq::App> {
    let frontend = app.env().get_or("FRONTEND_ORIGIN", "http://localhost:3000".to_owned())?;
    let logging = app.env().get_or("RUVORAQ_REQUEST_LOG", true)?;
    let timeout_ms = app.env().get_or("RUVORAQ_REQUEST_TIMEOUT_MS", 5000u64)?;
    Ok(app
        .request_logging(logging)
        .request_timeout(std::time::Duration::from_millis(timeout_ms))
        .cors(ruvoraq::Cors::new([frontend])?))
}

ruvoraq::bootstrap!(configure);
```

Keep one configure hook and one bootstrap invocation. This hook reads environment
values when selecting its defaults, preserving the user's overrides.

Allowed origins are complete HTTP(S) origins, including a port when needed.
Paths, queries, fragments, user credentials and wildcards are rejected.
The allowlist supports multiple origins and removes duplicates:

```rust
let cors = ruvoraq::Cors::new([
    "http://localhost:3000",
    "https://frontend.example.com",
])?;
```

Credentials are disabled by default. For deliberate cookie/credential support,
use `.credentials(true)` on this explicit allowlist. There is no wildcard-origin
mode. HTTP methods allowed by the policy are GET, HEAD, POST, PUT, PATCH and
DELETE; accepted request headers are content-type, authorization and x-request-id.
The ID response header is exposed to browser JavaScript.

Try an actual browser-origin request and a preflight:

```bash
curl -i http://127.0.0.1:8000/ -H 'Origin: http://localhost:3000'
curl -i -X OPTIONS http://127.0.0.1:8000/ \
  -H 'Origin: http://localhost:3000' \
  -H 'Access-Control-Request-Method: POST' \
  -H 'Access-Control-Request-Headers: content-type,x-request-id'
```

Allowed origins receive the appropriate access-control headers. Disallowed
origins receive no allow-origin header, so the browser refuses access.
Genuine preflights return an empty response without running handlers. Ordinary
OPTIONS requests without the preflight headers retain normal method errors.
Timeout responses keep the CORS headers so the frontend can read the error.

## ⏱️ Request timeouts

Use `RUVORAQ_REQUEST_TIMEOUT_MS` or `app.request_timeout(Duration)`.
For a manual builder, `.without_request_timeout()` removes the deadline.
A timeout produces HTTP 408:

```json
{
  "error": {
    "code": "request_timeout",
    "message": "Request timed out",
    "details": {}
  }
}
```

The deadline applies until the handler returns response headers, including
awaited extraction/work inside that future. It does not bound streaming response
bodies or idle connections. Tokio cancellation drops the pending future; it
cannot undo already committed writes, stop separately spawned tasks or forcibly
interrupt blocking code. Database connection/acquisition timeouts remain separate.

## 🧪 Verification and coverage

```bash
cargo test -p ruvoraq-web --test middleware
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
CARGO_LLVM_COV_TARGET_DIR=target/coverage-middleware \
  cargo llvm-cov -p ruvoraq-web --test middleware --json \
  --output-path target/middleware-coverage.json
python3 scripts/check_middleware_coverage.py target/middleware-coverage.json
python3 examples/app/tests/middleware.py
```

The gate requires **100% measured lines, functions and regions in the production
middleware module**. It does not exclude middleware code. This is not a claim of
100% whole-workspace coverage or proof that every possible runtime failure is
tested. Branch instrumentation is not enabled by this command. Separate
workspace coverage results are recorded in the [testing guide](testing_guid.md).

Tests cover validation boundaries, duplicate IDs, concurrency, errors/panics,
origin allowlists, credentials, preflight behavior, 405 preservation, cancellation,
fast responses, disabled options, safe tracing fields and startup config failures.
