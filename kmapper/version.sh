#!/bin/sh
# Print K-openMapper's version of the checked-out commit: <openMapper version>-k<N>.
#   openMapper version  Cargo.toml (upstream's, never edited here)
#   N                   the release tagged on this commit, or one more than the last
#                       v<openMapper version>-k* release (1 when there is none yet)
# Needs the tags (CI: fetch-depth: 0).
set -eu
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
base=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
last() { git tag -l "$@" "v$base-k*" 2>/dev/null | sed "s/^v$base-k//" | grep -E '^[0-9]+$' | sort -n | tail -1; }
n=$(last --points-at HEAD)
if [ -z "$n" ]; then
  n=$(last)
  if [ -n "$n" ]; then n=$((n + 1)); else n=1; fi
fi
echo "$base-k$n"
