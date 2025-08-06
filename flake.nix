{
  description = "Rust flake";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };
  outputs = { self, nixpkgs, ... }@inputs: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
  in {
    devShells.${system}.default = pkgs.mkShell rec {
      packages = with pkgs; [ rustc cargo ];
      nativeBuildInputs = with pkgs; [ pkg-config ];
      buildInputs = with pkgs; [ alsa-lib ];
      LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
    };
  };
}

