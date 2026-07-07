{ pkgs, lib, config, inputs, ... }:
{
  stdenv = pkgs.stdenvNoCC;

  overlays = [
    inputs.pebble.overlays.default
  ];

  packages = with pkgs; [
    cargo-show-asm
    cargo-bloat
    nodejs
    pebble-qemu
    pebble-tool
    pebble-toolchain-bin
    python3
    libiconv
    # clang
    # cargo-binutils
    llvmPackages.libclang.lib
  ];

  env = {
    PEBBLE_EXTRA_PATH = with pkgs; lib.makeBinPath [
      pebble-qemu
      pebble-toolchain-bin
    ];

    PEBBLE_EMULATOR = "emery";

    PEBBLE_BUILD_DIR = "${config.devenv.root}/build";

    # needed on darwin, might need an equivalent on linux?
    # needed so that @rpath/libLLVM.dylib resolves
    # DYLD_FALLBACK_LIBRARY_PATH="${config.languages.rust.toolchainPackage}/lib";
  };

  enterShell = ''
    export CC="arm-none-eabi-gcc"
    export LIBRARY_PATH=$LIBRARY_PATH:${pkgs.libiconv}/lib;
    export LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib"
  '';

  # export DYLD_FALLBACK_LIBRARY_PATH="${config.languages.rust.toolchainPackage}/lib"

  unsetEnvVars = ["CC" "CC_FOR_BUILD"];

  languages.rust = {
    enable = true;
    channel = "nightly";
    targets = [ "thumbv7m-none-eabi" ];
    components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" "rust-src" ];
  };
}
