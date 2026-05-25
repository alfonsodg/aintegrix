#!/bin/bash
# E2E smoke tests for AIntegriX API
# Run against: localhost or your deployed server
set -e

BASE="http://localhost:8050"
TOKEN="Authorization: Bearer ${AINTEGRIX_API_KEY:-test-key}"
PASS=0
FAIL=0

check() {
  local name="$1" expected="$2" actual="$3"
  if echo "$actual" | grep -qF "$expected"; then
    echo "  ✅ $name"
    PASS=$((PASS + 1))
  else
    echo "  ❌ $name (expected '$expected', got: $actual)"
    FAIL=$((FAIL + 1))
  fi
}

echo "=== AIntegriX E2E Tests ==="
echo ""

# 1. Health (no auth)
echo "[1] Health endpoints"
R=$(curl -s -o /dev/null -w "%{http_code}" "$BASE/health")
check "GET /health → 200" "200" "$R"

R=$(curl -s -o /dev/null -w "%{http_code}" -H "$TOKEN" "$BASE/readiness")
check "GET /readiness → 200" "200" "$R"

# 2. Auth
echo "[2] Authentication"
R=$(curl -s -o /dev/null -w "%{http_code}" "$BASE/api/v1/agents")
check "No token → 401" "401" "$R"

R=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer wrong" "$BASE/api/v1/agents")
check "Wrong token → 401" "401" "$R"

# 3. Agents
echo "[3] Agent endpoints"
R=$(curl -s -H "$TOKEN" "$BASE/api/v1/agents")
check "GET /agents has kiro" "kiro" "$R"
check "GET /agents has opencode" "opencode" "$R"
check "GET /agents has claude" "claude" "$R"

R=$(curl -s -H "$TOKEN" "$BASE/api/v1/agents/kiro/models")
check "GET /agents/kiro/models" "claude-opus" "$R"

R=$(curl -s -o /dev/null -w "%{http_code}" -H "$TOKEN" "$BASE/api/v1/agents/nonexistent/models")
check "GET /agents/nonexistent → 404" "404" "$R"

# 4. Agent status
echo "[4] Agent status"
R=$(curl -s -H "$TOKEN" "$BASE/api/v1/agents/status")
check "GET /agents/status has idle" "idle" "$R"
check "GET /agents/status has max_sessions" "max_sessions" "$R"

# 5. Sessions
echo "[5] Session lifecycle"
R=$(curl -s -X POST -H "$TOKEN" -H "Content-Type: application/json" \
  "$BASE/api/v1/sessions" -d '{"agent": "opencode"}')
check "POST /sessions → id" "opencode_" "$R"
SESSION=$(echo "$R" | python3 -c "import json,sys; print(json.load(sys.stdin).get('id',''))" 2>/dev/null)

if [ -n "$SESSION" ]; then
  R=$(curl -s -H "$TOKEN" "$BASE/api/v1/sessions/$SESSION")
  check "GET /sessions/{id} → active" "active" "$R"

  R=$(curl -s --max-time 60 -X POST -H "$TOKEN" -H "Content-Type: application/json" \
    "$BASE/api/v1/sessions/$SESSION/prompt" \
    -d '{"messages": [{"type": "text", "text": "say hello"}]}')
  check "POST /sessions/{id}/prompt → stop_reason" "end_turn" "$R"

  # Fork
  R=$(curl -s -X POST -H "$TOKEN" -H "Content-Type: application/json" \
    "$BASE/api/v1/sessions/$SESSION/fork" -d '{"target_agent": "claude"}')
  check "POST /sessions/{id}/fork → forked_from" "forked_from" "$R"

  # Close
  R=$(curl -s -o /dev/null -w "%{http_code}" -X DELETE -H "$TOKEN" "$BASE/api/v1/sessions/$SESSION")
  check "DELETE /sessions/{id} → 204" "204" "$R"
fi

# 6. Routing
echo "[6] Auto-routing"
R=$(curl -s -X POST -H "$TOKEN" -H "Content-Type: application/json" \
  "$BASE/api/v1/sessions" -d '{"auto_route": true, "prompt": "fix the React component"}')
check "auto_route React → copilot" "copilot" "$R"

R=$(curl -s -X POST -H "$TOKEN" -H "Content-Type: application/json" \
  "$BASE/api/v1/sessions" -d '{"auto_route": true, "prompt": "review security"}')
check "auto_route security → claude" "claude" "$R"

# 7. Invalid requests
echo "[7] Error handling"
R=$(curl -s -X POST -H "$TOKEN" -H "Content-Type: application/json" \
  "$BASE/api/v1/sessions" -d '{"agent": "nonexistent"}')
check "Invalid agent → agent_not_found" "agent_not_found" "$R"

R=$(curl -s -o /dev/null -w "%{http_code}" -X POST -H "$TOKEN" -H "Content-Type: application/json" \
  "$BASE/api/v1/sessions/fake_id/prompt" -d '{"messages": []}')
check "Invalid session → 404" "404" "$R"

# 8. Usage
echo "[8] Usage tracking"
R=$(curl -s -H "$TOKEN" "$BASE/api/v1/usage")
check "GET /usage → array" "[" "$R"

# 9. Webhook
echo "[9] Webhook"
R=$(curl -s -X POST -H "$TOKEN" -H "Content-Type: application/json" \
  "$BASE/api/v1/webhooks/git" -d '{"object_kind": "push"}')
check "Webhook push → ignored" "ignored" "$R"

R=$(curl -s -X POST -H "$TOKEN" -H "Content-Type: application/json" \
  "$BASE/api/v1/webhooks/git" \
  -d '{"object_kind": "merge_request", "object_attributes": {"action": "open", "title": "test", "source_branch": "feat", "target_branch": "dev"}}')
check "Webhook MR → accepted" "accepted" "$R"

# 10. MCP
echo "[10] MCP endpoint"
R=$(curl -s -X POST -H "$TOKEN" -H "Content-Type: application/json" \
  "$BASE/mcp" -d '{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}')
check "POST /mcp initialize → result" "result" "$R"

R=$(curl -s -X POST -H "$TOKEN" -H "Content-Type: application/json" \
  "$BASE/mcp" -d '{"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}')
check "POST /mcp tools/list → acp_prompt" "acp_prompt" "$R"

# MCP full flow: create → prompt → close
echo "[11] MCP full flow (create → prompt → close)"
R=$(curl -s -X POST -H "$TOKEN" -H "Content-Type: application/json" \
  "$BASE/mcp" -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"acp_create_session","arguments":{"agent":"opencode"}}}')
check "MCP create_session → Session created" "Session created" "$R"
MCP_SESSION=$(echo "$R" | python3 -c "import json,sys; t=json.load(sys.stdin)['result']['content'][0]['text']; print(t.split('Session created: ')[1].split(' ')[0])" 2>/dev/null)

if [ -n "$MCP_SESSION" ]; then
  R=$(curl -s --max-time 60 -X POST -H "$TOKEN" -H "Content-Type: application/json" \
    "$BASE/mcp" -d "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"acp_prompt\",\"arguments\":{\"session_id\":\"${MCP_SESSION}\",\"message\":\"say hello\"}}}")
  check "MCP prompt → Agent responded" "Agent responded" "$R"

  R=$(curl -s -X POST -H "$TOKEN" -H "Content-Type: application/json" \
    "$BASE/mcp" -d "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\"params\":{\"name\":\"acp_close_session\",\"arguments\":{\"session_id\":\"${MCP_SESSION}\"}}}")
  check "MCP close_session → closed" "closed" "$R"
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
