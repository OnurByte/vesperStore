# AGENTS.md

## scope

This repository owns Vesper Store. It is a standalone application consumed by
Vesper and may be reused by other NixOS configurations.

## source of truth

`docs/MARKETPLACE.md` owns product, source, transaction and UX behavior.
Current code is implementation truth. Target prose is not implementation proof.

## implementation rules

- Keep the application name `Vesper Store`, app id `io.vesper.Store` and binary `vesper-store`.
- Base the final Store on the pinned `snowfallorg/nix-software-center` source.
- Keep package mutations in the Rust transaction core, not presentation code.
- Nixpkgs from the consuming flake is the default source.
- Flathub stays opt-in and user scoped.
- Never execute arbitrary Nix expressions or command text from catalogue data.
- Failed transactions must leave the previous effective manifest and profile active.
- Do not add first-party Python.

## checks

- compile and run Rust tests for changed backend code
- lint changed QML while the transitional QML shell exists
- parse changed Nix files
- build `.#vesper-store`
- keep specification, local build and live install evidence separate
