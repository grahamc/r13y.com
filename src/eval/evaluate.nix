{ revision, sha256, subfile, attrsJSON }:
let
  attrs = builtins.fromJSON attrsJSON;

  src = builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/${revision}.tar.gz";
    inherit sha256;
  };

  lib = import "${src}/lib";

  toImport = "${src}/${subfile}";

  imported = import
    (builtins.trace "Importing: ${toImport}" toImport);

  called = imported {
    # Todo: pass system, once
    # https://github.com/NixOS/nixpkgs/blob/b8d7c0cab5e5e5ea8002ce4ad38f336df12841fe/nixos/release-combined.nix#L15
    # allows specifying the nixpkgs system. when fixed, make sure to
    # pass --pure-eval when evaluating this file
  };

  tracedEval = attr:
    if lib.hasAttrByPath attr called
    then builtins.trace "Found «${toString attr}» in ${subfile}" (lib.attrByPath attr null called)
    else builtins.trace "Missing «${toString attr}» in ${subfile}" null;

in builtins.map tracedEval attrs
