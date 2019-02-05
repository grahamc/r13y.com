#!/usr/bin/env nix-shell
#!nix-shell -i bash -p bc

{
    cd nixpkgs
    rev=$(git rev-parse HEAD)
    cd ..

    reproducible=$(cat ./reproducibility-log | grep '^reproducible' | wc -l)
    total=$(cat ./reproducibility-log | wc -l)
    percent=$(printf "%.3f" "$(echo "($reproducible / $total) * 100" | bc -l)")

    cat <<EOF
<html>
<head>
<title>NixOS's minimal ISO is $percent% reproducible!</title>
<meta name="description" content="nixos-unstable's iso_minimal.x86_64-linux build is $percent% reproducible!" />

<!-- Twitter Card data -->
<meta name="twitter:card" value="summary">

<!-- Open Graph data -->
<meta property="og:title" content="Is NixOS Reproducible?" />
<meta property="og:type" content="article" />
<meta property="og:url" content="https://r13y.com/" />
<meta property="og:image" content="https://nixos.org/logo/nixos-logo-only-hires.png" />
<meta property="og:description" content="nixos-unstable's iso_minimal.x86_64-linux build is $percent% reproducible!" />
</head>
<body>
<h1>Is NixOS Reproducible?</h1>
<h2>Currently tracking: <code>nixos-unstable</code>'s
    <code>iso_minimal</code> job for <code>x86_64-linux</code>.</h2>
<p>Build via:</p>
<pre>
git clone https://github.com/nixos/nixpkgs.git
cd nixpkgs
git checkout $rev
nix-build ./nixos/release-combined.nix -A nixos.iso_minimal.x86_64-linux
</pre>

<h1 style="color: green">$reproducible / $total ($percent%) are reproducible!</h1>
<hr>
<h3>unreproduced paths</h3>
EOF

    for drv in $(cat ./reproducibility-log | awk '$1 == "unreproducible" { print $2; }'); do
        cat <<EOF
<li><a href="./diff/$(basename "$drv").html">(diffoscope)</a> <a href=".$drv">(drv)</a> <code>$drv</code></li>
EOF
    done

    cat <<EOF
<hr />
<small>Generated at $(TZ=UTC date) from
<a href="https://github.com/grahamc/r13y.com">https://github.com/grahamc/r13y.com</a></small>
<center><img style="max-width: 100px" src="https://nixos.org/logo/nixos-logo-only-hires.png" /></center>
</body></html>
EOF
}
