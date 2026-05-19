#!/usr/bin/env sh
set -eu

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

if [ "$PROFILE" = "debug" ]; then
  echo "Building gd debug binary..."
  cargo build
else
  echo "Building gd release binary..."
  cargo build --release
fi

echo "Build completed."
