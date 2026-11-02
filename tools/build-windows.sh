#!/usr/bin/env bash
# Cross-build da Linux: richiede mingw-w64, nsis e il target rust x86_64-pc-windows-gnu.
# Su Windows usa invece: npm run build
set -euo pipefail
cd "$(dirname "$0")/.."
npx tauri build --target x86_64-pc-windows-gnu
