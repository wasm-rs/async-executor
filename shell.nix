let
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  pkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
  rustChannel = (pkgs.rustChannelOf { channel = "stable";  });
  rust = (rustChannel.rust.override {
    targets = ["wasm32-unknown-unknown" "wasm32-wasi"];
  });
in
pkgs.stdenv.mkDerivation rec {
  name = "wasm-rs-async-channel-shell";

  buildInputs = with pkgs; [ rust nodejs
                             openssl.dev pkgconfig # for cargo-release
  ];

  shellHook = ''
    # Useful for ensuring cargo tools are available (like cargo-do, for example)
    export PATH=$PATH:$HOME/.cargo/bin
  '';

}
