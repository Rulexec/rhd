#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

RHD_BIN="../target/debug/rhd"
SOCKET_PATH="rhd.sock"
DAEMON_PID=""

cleanup() {
    if [ -n "$DAEMON_PID" ]; then
        kill "$DAEMON_PID" 2>/dev/null || true
        wait "$DAEMON_PID" 2>/dev/null || true
    fi
    rm -f "$SOCKET_PATH"
}

trap cleanup EXIT

echo "Building rhd..."
cd ..
cargo build --quiet
cd "$SCRIPT_DIR"

echo "Starting daemon..."
"$RHD_BIN" daemon --models-dir models --scenarios-dir scenarios &
DAEMON_PID=$!

echo "Waiting for daemon to start..."
for i in {1..10}; do
    if [ -S "$SOCKET_PATH" ]; then
        break
    fi
    sleep 0.1
done

if [ ! -S "$SOCKET_PATH" ]; then
    echo "ERROR: Daemon failed to create socket"
    exit 1
fi

echo "Running e2e_test scenario..."
"$RHD_BIN" run e2e_test
EXIT_CODE=$?

if [ $EXIT_CODE -eq 0 ]; then
    echo "SUCCESS: e2e test passed"
else
    echo "FAILED: e2e test failed with exit code $EXIT_CODE"
fi

exit $EXIT_CODE
