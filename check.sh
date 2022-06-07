#!/usr/bin/env nix-shell
#!nix-shell -i bash ./default.nix -I nixpkgs=channel:nixos-unstable-small

set -eux

#export LANG=en_US.UTF-8
#export LOCALE_ARCHIVE=/run/current-system/sw/lib/locale/locale-archive

function nixpkgs_rev() (
    curl https://channels.nix.gsc.io/nixos-unstable-small/latest | cut -d' ' -f1
)

function main() {
    export SUBSET="$1"
    export REPORT_NAME="$2"
    export REV=$(nixpkgs_rev)
    export HASH=$(nix-prefetch-url --unpack "https://github.com/NixOS/nixpkgs/archive/${REV}.tar.gz")

    export RUST_BACKTRACE=1

    (
        unset RUST_LOG
        cargo build
    )

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

    mv ./report "./$REPORT_NAME"

    tar -cJf "./$REPORT_NAME.tar.xz" "./$REPORT_NAME"
    buildkite-agent artifact upload "./$REPORT_NAME.tar.xz"
}

main "$1" "$2"
