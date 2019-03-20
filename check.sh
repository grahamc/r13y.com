#!/usr/bin/env nix-shell
#!nix-shell -i bash ./default.nix -I nixpkgs=channel:nixos-unstable-small

set -eux

export LANG=en_US.UTF-8
export LOCALE_ARCHIVE=/run/current-system/sw/lib/locale/locale-archive

function nixpkgs_rev() (
    curl https://channels.nix.gsc.io/nixos-unstable-small/latest | cut -d' ' -f1
)

function main() {
    export REV=$(nixpkgs_rev)
    export HASH=$(nix-prefetch-url --unpack "https://github.com/NixOS/nixpkgs/archive/${REV}.tar.gz")
    cd ./r13y
    export RUST_BACKTRACE=1
    export RUST_LOG=debug
    cargo run --bin check -- "$REV" "$HASH"
    cargo run --bin report -- "$REV" "$HASH"
}

main
