{
  description = "blockvisor-api";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{ nixpkgs, flake-parts, ... }:

    flake-parts.lib.mkFlake { inherit inputs; } {
      debug = true;
      systems = [
        "x86_64-linux"
        "aarch64-darwin"
      ];

      perSystem =
        { pkgs, system, ... }:
        {
          formatter = nixpkgs.legacyPackages.${system}.nixfmt-rfc-style;

          devShells.default = pkgs.mkShell {
            packages = with pkgs; [
              openssl
              pgcli
              pkg-config
              postgresql # TODO: libpq from https://github.com/NixOS/nixpkgs/pull/294504
              protobuf
            ];
          };
        };
    };
}
