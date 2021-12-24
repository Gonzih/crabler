let
   pkgs = import <nixpkgs> {};
in pkgs.stdenv.mkDerivation rec {
  name = "crabler-dev";
  buildInputs = with pkgs; [
    stdenv
    glib
    pkgconfig
    rustup
    cargo
    curl
    zlib
  ];
}
