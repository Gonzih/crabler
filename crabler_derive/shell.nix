let
   pkgs = import <nixpkgs> {};
in pkgs.stdenv.mkDerivation rec {
  name = "vuln-scanner-dev";
  buildInputs = with pkgs; [
    stdenv
    glib
    pkgconfig
    rustup
    cargo
    curl
    postgresql_11
  ];
}
