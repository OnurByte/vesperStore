# Vesper Store

Vesper Store is the application discovery and installation surface extracted
from the Vesper workstation repository.

The current application packages a local AppStream catalogue, searches it with
SQLite FTS5 and installs selected Nixpkgs applications through a dedicated,
manifest-backed Nix profile. The Qt/QML frontend never executes catalogue
commands directly; the Rust core re-resolves and validates every install.

```bash
nix build .#vesper-store
```

For a sibling Vesper checkout, add the Store as a flake input following
Vesper's Nixpkgs input, then include its package in the Home Manager profile:

```nix
vesper-store = {
  url = "path:../vesper-store";
  inputs.nixpkgs.follows = "nixpkgs";
};

inputs.vesper-store.packages.${pkgs.system}.vesper-store
```

Vesper consumes this project as an external flake. Installed application
management remains in Vesper Settings.
