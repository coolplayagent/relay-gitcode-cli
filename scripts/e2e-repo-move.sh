#!/usr/bin/env sh
set -eu

: "${GITCODE_TOKEN:?set GITCODE_TOKEN to a GitCode token with access to move the test repository}"
: "${GD_E2E_SOURCE_REPO:?set GD_E2E_SOURCE_REPO to owner/repo for a disposable test repository}"
: "${GD_E2E_TARGET_OWNER:?set GD_E2E_TARGET_OWNER to the destination user or organization}"
: "${GD_E2E_TARGET_NAME:?set GD_E2E_TARGET_NAME to the temporary destination repository name}"

GD_BIN="${GD_BIN:-target/debug/gd}"
GD_E2E_RESTORE="${GD_E2E_RESTORE:-1}"

source_owner="${GD_E2E_SOURCE_REPO%%/*}"
source_name="${GD_E2E_SOURCE_REPO#*/}"
if [ "$source_owner" = "$GD_E2E_SOURCE_REPO" ] || [ -z "$source_owner" ] || [ -z "$source_name" ]; then
  echo "GD_E2E_SOURCE_REPO must be owner/repo" >&2
  exit 2
fi

target_repo="${GD_E2E_TARGET_OWNER}/${GD_E2E_TARGET_NAME}"

echo "Moving ${GD_E2E_SOURCE_REPO} -> ${target_repo}"
"$GD_BIN" repo move "$GD_E2E_SOURCE_REPO" "$target_repo" --json >/dev/null
"$GD_BIN" repo view "$target_repo" --json >/dev/null

if [ "$GD_E2E_RESTORE" = "1" ]; then
  echo "Restoring ${target_repo} -> ${GD_E2E_SOURCE_REPO}"
  "$GD_BIN" repo move "$target_repo" "$GD_E2E_SOURCE_REPO" --json >/dev/null
  "$GD_BIN" repo view "$GD_E2E_SOURCE_REPO" --json >/dev/null
else
  echo "Leaving repository at ${target_repo}"
fi

echo "Repository move E2E completed"
