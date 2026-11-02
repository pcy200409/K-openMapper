#!/bin/sh
# Write a K-openMapper version (kmapper/version.sh) into Cargo.toml and Cargo.lock for a build
# (CI only, never committed): the program, its package names and the updater all read it.
set -eu
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
v="$1"
# (awk, not sed -i: the same on Linux, macOS and Git Bash)
awk -v v="$v" '!done && /^version = / { $0 = "version = \"" v "\""; done = 1 } { print }' Cargo.toml > Cargo.toml.new
mv Cargo.toml.new Cargo.toml
awk -v v="$v" 'prev == "name = \"openmapper\"" && /^version = / { $0 = "version = \"" v "\"" } { print; prev = $0 }' Cargo.lock > Cargo.lock.new
mv Cargo.lock.new Cargo.lock
grep -n "^version = \"$v\"" Cargo.toml
grep -c "^version = \"$v\"" Cargo.lock
