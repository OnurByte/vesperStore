{
  description = "Vesper Store";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nix-software-center = {
      url = "github:snowfallorg/nix-software-center/181c1c61eab79130879257550dba0b36bd6bb8c9";
      flake = false;
    };
    nixos-appstream-data = {
      url = "github:korfuri/nixos-appstream-data/0465d42a4433faa63b7a5eb0b8d397937c9fc13a";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      nix-software-center,
      nixos-appstream-data,
      ...
    }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      formatter.${system} = pkgs.nixfmt-rfc-style;
      packages.${system} = rec {
        vesper-store = pkgs.callPackage ./package.nix {
          nixosAppstreamData = nixos-appstream-data.packages.${system}.nixos-appstream-data;
          nixpkgsRevision = nixpkgs.rev or null;
          upstreamRevision = "181c1c61eab79130879257550dba0b36bd6bb8c9";
          upstreamSource = nix-software-center;
        };
        default = vesper-store;
      };
    };
}
