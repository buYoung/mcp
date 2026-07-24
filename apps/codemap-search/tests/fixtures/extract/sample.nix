{ lib, pkgs, callPackage, ... }:
let
  privateValue = 1;
  helper = value: lib.transform value;
  package = callPackage ./package.nix {};
in {
  services.api.enable = true;
  "quoted".port = 5000;
  inherit helper;
  inherit (pkgs) nginx;
  imported = import ./module.nix;
  target = pkgs.stdenv.mkDerivation { name = "demo"; };
  result = helper package;
}
