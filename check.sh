#!/usr/bin/env nix-shell
#!nix-shell -i bash ./default.nix -I nixpkgs=channel:nixos-19.09

set -eux

#export LANG=en_US.UTF-8
#export LOCALE_ARCHIVE=/run/current-system/sw/lib/locale/locale-archive

function nixpkgs_rev() (
    curl https://channels.nix.gsc.io/nixos-unstable-small/latest | cut -d' ' -f1
)

function main() {
    export REV=$(nixpkgs_rev)
    export HASH=$(nix-prefetch-url --unpack "https://github.com/NixOS/nixpkgs/archive/${REV}.tar.gz")
    export SUBSET=nixos:nixos.iso_plasma5.x86_64-linux
    export RUST_BACKTRACE=1
    (
        unset RUST_LOG
        cargo build
    )

    # SUBSET="nixpkgs:stdenv.__bootPackages.stdenv.__bootPackages.stdenv.__bootPackages.stdenv.__bootPackages.stdenv.__bootPackages.binutils"
    cargo run -- \
        --subset "$SUBSET" \
        --rev "$REV" \
        --sha256 "$HASH" \
        --max-cores 48 \
        --max-cores-per-job 4 \
        check

    cargo run -- \
        --subset "$SUBSET" \
        --rev "$REV" \
        --sha256 "$HASH" \
        report

    tar -cJf ./report.tar.xz ./report
    buildkite-agent artifact upload ./report.tar.xz
}

main
