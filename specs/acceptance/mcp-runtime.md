# MCP Runtime Acceptance

## Purpose
This document defines the currently shipped MCP runtime baseline for the authenticated API bridge. It is intentionally bounded to JSON-RPC-over-HTTP client interoperability and permission-scoped retrieval tools; it does not claim a production MCP transport/session manager beyond this HTTP endpoint.

## Grounding
- `POST /api/v1/mcp` in `crates/api/src/main.rs`
- `GET /api/v1/mcp/manifest`, `GET /api/v1/mcp/tools`, and `POST /api/v1/mcp/tools/call` in `crates/api/src/main.rs`
- Core MCP tool definitions and execution in `crates/mcp/src/lib.rs`
- Focused API regression coverage around MCP authentication, initialize, tools/list, tools/call, batch handling, and permission-scoped repository visibility.

## Acceptance scenarios
1. MCP HTTP endpoints require an authenticated local session and fail closed with `401` when unauthenticated.
2. `initialize` returns the advertised MCP protocol version, server info, and tools capability metadata.
3. `tools/list` returns the retrieval tool definitions, including the HTTP/API-required `repo_id` argument on repository-content tools.
4. `tools/call` returns MCP-shaped text content plus structured content for successful retrieval calls, and MCP-shaped `isError: true` results for tool execution errors.
5. JSON-RPC schema errors return JSON-RPC errors instead of panics or partial tool execution.
6. `POST /api/v1/mcp` is fail-closed on media type: authenticated JSON-RPC requests must use `Content-Type: application/json` (parameters such as `charset=utf-8` are accepted), and non-JSON media types return `415` before request parsing or tool execution.
7. JSON-RPC batch requests are supported on `POST /api/v1/mcp`: the endpoint evaluates each request in order, returns an array of responses, and omits response entries for JSON-RPC notifications such as `notifications/initialized`.
8. Permission-scoped repository behavior is fail closed: callers can only bind `repo_id` to repositories visible through their authenticated organization membership; hidden repository calls through `POST /api/v1/mcp` remain on the MCP/JSON-RPC transport and return an MCP `isError: true` permission result instead of widening retrieval, direct HTTP tool calls continue to return `404`, and `list_repos` filters results to the authenticated visible repository set.

## Explicit deferrals
The current MCP runtime acceptance does **not** claim:
- stdio, SSE, or streamable-HTTP session lifecycle parity beyond this authenticated JSON-RPC-over-HTTP API endpoint
- durable MCP client/session registration
- unauthenticated/public MCP access
- write tools or administrative tools
- production-grade MCP observability beyond API responses and test evidence
