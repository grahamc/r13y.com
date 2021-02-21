#!/usr/bin/env nix-shell
#!nix-shell -i bash -p awscli -I nixpkgs=channel:nixos-unstable-small

REPORT_NAME=$1
UPLOAD_DEST=$2

buildkite-agent artifact download "$REPORT_NAME.tar.xz" ./
tar -xf "./$REPORT_NAME".tar.xz

aws s3 cp "$REPORT_NAME" "s3://r13y-com$UPLOAD_DEST" --recursive --acl public-read \
    --cache-control "public; max-age=3600"
