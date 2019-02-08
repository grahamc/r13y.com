with import <nixpkgs> {};
mkShell {
  buildInputs = [
    bc
    coreutils
    (diffoscope.override { enableBloat = true; })
    findutils
    git
    jq
    nix
  ];
}
