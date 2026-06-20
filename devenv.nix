{ pkgs, lib, config, inputs, ... }:
let
  crate2nixTools = pkgs.callPackage "${inputs.crate2nix}/tools.nix" { };
  cargoNix = path: crate: (pkgs.callPackage (crate2nixTools.generatedCargoNix { name = crate; src = path; }) { }).workspaceMembers.${crate}.build;
  cargo-pebble = cargoNix inputs.cargo-pebble "pebble-cli";
in
{
  overlays = [
    inputs.pebble.overlays.default
  ];

  packages = with pkgs; [
    cargo-pebble
    flip-link
    cargo-show-asm
    cargo-bloat
    nodejs
    pebble-qemu
    pebble-tool
    pebble-toolchain-bin
    python3
    cargo-binutils
    clang
  ];

  env.PEBBLE_EXTRA_PATH = with pkgs; lib.makeBinPath [
    pebble-qemu
    pebble-toolchain-bin
  ];

  env.PEBBLE_EMULATOR = "emery";

  languages.rust = {
    enable = true;
    channel = "nightly";
    targets = [ "thumbv7m-none-eabi" ];
    components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" "rust-src" "llvm-tools" ];
  };
}
