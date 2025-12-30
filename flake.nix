{
  description = "talaria dev shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            rustc cargo rustfmt clippy rust-analyzer
            pkg-config cmake

            # clang-sys / bindgen
            llvmPackages_latest.llvm
            llvmPackages_latest.clang
            llvmPackages_latest.libclang

            # opencv + gui runtime deps
            opencv4
            gtk3
          ] ++ (with pkgs.xorg; [
            libX11 libXext libXrender libXrandr libXi libXtst
          ]);

          LIBCLANG_PATH = "${pkgs.llvmPackages_latest.libclang.lib}/lib";
          LLVM_CONFIG_PATH = "${pkgs.llvmPackages_latest.llvm.dev}/bin/llvm-config";
          PKG_CONFIG_PATH = "${pkgs.opencv4.dev}/lib/pkgconfig";
        };
      });
}
