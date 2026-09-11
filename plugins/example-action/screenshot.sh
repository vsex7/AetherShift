#!/usr/bin/env bash
set -e
# Simple action handler responding to AetherShift Action Execution Protocol
input=$(cat)
echo '{"status":"ok","message":"Screenshot captured"}'
