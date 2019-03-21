#!/usr/bin/env nix-shell
#!nix-shell -i bash ./default.nix -I nixpkgs=channel:nixos-unstable-small

set -eux

#export LANG=en_US.UTF-8
#export LOCALE_ARCHIVE=/run/current-system/sw/lib/locale/locale-archive

function nixpkgs_rev() (
    curl https://channels.nix.gsc.io/nixos-unstable-small/latest | cut -d' ' -f1
)

function main() {
    export REV=$(nixpkgs_rev)
    export HASH=$(nix-prefetch-url --unpack "https://github.com/NixOS/nixpkgs/archive/${REV}.tar.gz")
    export RUST_BACKTRACE=1
    (unset RUST_LOG; cargo build)

    cargo run --bin check -- "$REV" "$HASH" --one
    cargo run --bin report -- "$REV" "$HASH"
    rsync -e "ssh -i /etc/r13y-ssh-private" -r ./report/ r13y@r13y.com:r13y.com
}

main
