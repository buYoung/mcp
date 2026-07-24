{ lib, callPackage, ... }:
let
  helper = value: lib.transform value;
  package = callPackage ./package.nix {};
  caller = helperArg: helper (helperArg package);
in {
  result = caller package;
  imported = builtins.import ./module.nix;
  inherit (lib) optional;
}
