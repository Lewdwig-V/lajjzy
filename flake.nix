{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, crane, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;
        lajjzy = craneLib.buildPackage {
          src = craneLib.cleanCargoSource ./.;
          # jj-lib uses gix (pure Rust git), no native deps needed.
          # If builds fail on specific systems, add deps here.
        };
      in {
        packages.default = lajjzy;
        apps.default = flake-utils.lib.mkApp { drv = lajjzy; };
        devShells.default = craneLib.devShell {
          packages = with pkgs; [ jujutsu ];
        };
      });
}
