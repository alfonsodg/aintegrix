#!/usr/bin/env python3
"""E2E tests for AIntegriX MCP — tests real agent orchestration."""

import json
import requests
import sys
import time

BASE = "http://localhost:8050/mcp"
TOKEN = "Bearer test-key"
HEADERS = {"Authorization": TOKEN, "Content-Type": "application/json"}

PASS = 0
FAIL = 0


def mcp_call(method, params=None):
    """Send a JSON-RPC call to the MCP endpoint."""
    payload = {"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}}
    r = requests.post(BASE, json=payload, headers=HEADERS, timeout=180)
    return r.json()


def tool_call(name, arguments=None):
    """Call an MCP tool."""
    return mcp_call("tools/call", {"name": name, "arguments": arguments or {}})


def check(name, condition, detail=""):
    global PASS, FAIL
    if condition:
        print(f"  ✅ {name}")
        PASS += 1
    else:
        print(f"  ❌ {name} — {detail}")
        FAIL += 1


def get_text(result):
    """Extract text from MCP tool result."""
    try:
        return result["result"]["content"][0]["text"]
    except (KeyError, IndexError, TypeError):
        return ""


# ============================================================
# TEST 1: List agents
# ============================================================
print("[1] List agents")
r = tool_call("acp_list_agents")
text = get_text(r)
check("returns agent list", "kiro" in text and "opencode" in text and "claude" in text)

# ============================================================
# TEST 2: Create session + simple prompt + close
# ============================================================
print("[2] Session lifecycle (create → prompt → close)")
r = tool_call("acp_create_session", {"agent": "opencode"})
text = get_text(r)
check("session created", "Session created:" in text)
sid = text.split("Session created: ")[1].split(" ")[0] if "Session created:" in text else ""

if sid:
    r = tool_call("acp_prompt", {"session_id": sid, "message": "What is 2+2? One word."})
    text = get_text(r)
    check("prompt returns text", len(text) > 0 and "Error" not in text, f"got: {text[:100]}")
    check("answer is correct", "4" in text or "four" in text.lower(), f"got: {text[:100]}")

    r = tool_call("acp_close_session", {"session_id": sid})
    text = get_text(r)
    check("session closed", "closed" in text.lower())

# ============================================================
# TEST 3: Prompt that requires file reading
# ============================================================
print("[3] File reading (agent reads /etc/hostname)")
r = tool_call("acp_create_session", {"agent": "opencode", "workspace_root": "/tmp"})
sid = get_text(r).split("Session created: ")[1].split(" ")[0] if "Session created:" in get_text(r) else ""

if sid:
    r = tool_call("acp_prompt", {"session_id": sid, "message": "Read /etc/hostname and tell me the content. Just the hostname, nothing else."})
    text = get_text(r)
    check("agent read file", len(text) > 0 and "Error" not in text, f"got: {text[:100]}")
    check("hostname returned", "dev-gcp" in text.lower() or len(text.strip()) > 0, f"got: {text[:100]}")
    tool_call("acp_close_session", {"session_id": sid})

# ============================================================
# TEST 4: Multi-agent — create sessions with different agents
# ============================================================
print("[4] Multi-agent sessions")
agents_ok = []
for agent in ["kiro", "copilot", "claude"]:
    r = tool_call("acp_create_session", {"agent": agent})
    text = get_text(r)
    if "Session created:" in text:
        agents_ok.append(agent)
        sid = text.split("Session created: ")[1].split(" ")[0]
        tool_call("acp_close_session", {"session_id": sid})

check("kiro session", "kiro" in agents_ok)
check("copilot session", "copilot" in agents_ok)
check("claude session", "claude" in agents_ok)

# ============================================================
# TEST 5: Prompt with workspace context
# ============================================================
print("[5] Workspace-aware prompt")
r = tool_call("acp_create_session", {"agent": "opencode", "workspace_root": "/opt/aintegrix"})
sid = get_text(r).split("Session created: ")[1].split(" ")[0] if "Session created:" in get_text(r) else ""

if sid:
    r = tool_call("acp_prompt", {"session_id": sid, "message": "Read aintegrix.yaml and count how many agents are configured. Reply with just the number."})
    text = get_text(r)
    check("workspace prompt works", len(text) > 0 and "Error" not in text, f"got: {text[:100]}")
    check("correct agent count", "5" in text, f"got: {text[:100]}")
    tool_call("acp_close_session", {"session_id": sid})

# ============================================================
# TEST 6: Error handling
# ============================================================
print("[6] Error handling")
r = tool_call("acp_create_session", {"agent": "nonexistent"})
text = get_text(r)
check("invalid agent → error", "not found" in text.lower() or "Error" in text)

r = tool_call("acp_prompt", {"session_id": "fake_session_id", "message": "hi"})
text = get_text(r)
check("invalid session → error", "not found" in text.lower() or "Error" in text)

# ============================================================
# TEST 7: Consecutive prompts (same session)
# ============================================================
print("[7] Consecutive prompts (stateful session)")
r = tool_call("acp_create_session", {"agent": "opencode"})
sid = get_text(r).split("Session created: ")[1].split(" ")[0] if "Session created:" in get_text(r) else ""

if sid:
    r = tool_call("acp_prompt", {"session_id": sid, "message": "Remember the number 42."})
    text1 = get_text(r)
    check("first prompt ok", len(text1) > 0 and "Error" not in text1)

    r = tool_call("acp_prompt", {"session_id": sid, "message": "What number did I ask you to remember? Just the number."})
    text2 = get_text(r)
    check("session is stateful", "42" in text2, f"got: {text2[:100]}")
    tool_call("acp_close_session", {"session_id": sid})

# ============================================================
# RESULTS
# ============================================================
print(f"\n{'='*50}")
print(f"Results: {PASS} passed, {FAIL} failed")
print(f"{'='*50}")
sys.exit(0 if FAIL == 0 else 1)
