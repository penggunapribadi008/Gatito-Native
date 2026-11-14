#!/bin/bash
# Semua langkah build APK ada di sini, supaya build.yml tidak perlu diubah-ubah lagi.
set -e
VER=$(grep -m1 '^version' ../pc/Cargo.toml | cut -d'"' -f2)
OUT="gatito-stream-tool-v$VER.apk"
ARGS="-PappVersion=$VER -PappCode=${GITHUB_RUN_NUMBER:-1} -PupdateRepo=${GITHUB_REPOSITORY:-}"
if [ -n "$KEYSTORE_B64" ] && [ -n "$KEYSTORE_PASSWORD" ]; then
  echo "Membuat APK rilis yang ditandatangani"
  echo "$KEYSTORE_B64" | base64 -d > /tmp/sd.keystore
  export KEYSTORE_FILE=/tmp/sd.keystore
  gradle assembleRelease $ARGS --no-daemon
  cp app/build/outputs/apk/release/app-release.apk "$OUT"
else
  echo "Kunci belum diisi, membuat APK debug"
  gradle assembleDebug $ARGS --no-daemon
  cp app/build/outputs/apk/debug/app-debug.apk "$OUT"
fi
