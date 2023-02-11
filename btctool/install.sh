#!/bin/bash

set -e

script_path="$(dirname "$0")"
script_path="$(readlink -f "$script_path")"
cd "$script_path/btctool"

# gmp needed by python library fastecdsa which is used by bitcoinlib.
brew install gmp
CFLAGS=-I/opt/homebrew/opt/gmp/include LDFLAGS=-L/opt/homebrew/opt/gmp/lib poetry install
