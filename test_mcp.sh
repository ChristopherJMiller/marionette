#!/usr/bin/env bash
# Simple MCP protocol test for marionette server

set -e

MARIONETTE="./target/debug/marionette"

echo "=== Testing Marionette MCP Server ===" >&2
echo "" >&2

# Create a test session with proper MCP handshake
OUTPUT=$({
    # 1. Initialize
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}'
    sleep 0.1

    # 2. Initialized notification (no response expected)
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    sleep 0.1

    # 3. List tools
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
    sleep 0.2

    # 4. Call window_list tool
    echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"window_list","arguments":{}}}'
    sleep 0.5

} | $MARIONETTE 2>/tmp/marionette-stderr.log)

echo "1. Initialize response:" >&2
echo "$OUTPUT" | sed -n '1p' | jq -c '{serverInfo: .result.serverInfo}' >&2
echo "" >&2

echo "2. Tools list ($(echo "$OUTPUT" | sed -n '2p' | jq '.result.tools | length') tools):" >&2
echo "$OUTPUT" | sed -n '2p' | jq -r '.result.tools[] | "  - \(.name): \(.description)"' >&2
echo "" >&2

echo "3. Window list result:" >&2
echo "$OUTPUT" | sed -n '3p' | jq -r '.result.content[0].text'

echo "" >&2
echo "=== Server logs (stderr) ===" >&2
cat /tmp/marionette-stderr.log >&2
