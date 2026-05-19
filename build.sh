#!/usr/bin/env sh
set -eu

BINARY="gd"
SCRIPT_DIR=$(CDPATH= cd "$(dirname "$0")" && pwd)

PROFILE="release"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --debug)
      PROFILE="debug"
      shift
      ;;
    -h|--help)
      echo "Usage: ./build.sh [--debug]"
      exit 0
      ;;
    *)
      echo "[Error] unexpected argument: $1"
      exit 2
      ;;
  esac
done

cd "$SCRIPT_DIR"

if [ "$PROFILE" = "debug" ]; then
  TARGET_DIR="debug"
  echo "Building $BINARY debug binary..."
  cargo build --bin "$BINARY"
else
  TARGET_DIR="release"
  echo "Building $BINARY release binary..."
  cargo build --release --bin "$BINARY"
fi

echo "Build completed: target/$TARGET_DIR/$BINARY"
