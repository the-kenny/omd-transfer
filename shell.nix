let
  pkgs = import <nixpkgs> {};
  stdenv = pkgs.stdenv;
in rec {
  rustEnv = stdenv.mkDerivation rec {
    name = "rust-env";
    version = "1.2.3.4";
    src = ./.;
    buildInputs = with pkgs; [ pkgconfig dbus rustc cargo ];

    SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
  };
 }
