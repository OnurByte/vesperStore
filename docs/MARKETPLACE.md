# Vesper Store

Status: **spec**

This document is the single source of truth for Vesper Store architecture.
It defines the target product and transaction contract. It is not proof that every capability below is implemented.

Current implementation is partial: Vesper Store has a native Qt/QML application shell, a Rust transaction backend and a packaged SQLite catalogue generated from the pinned `nix-software-center` AppStream data source. Catalogue status rejects incomplete schemas and incoherent metadata, local FTS5 search is bounded, and Nixpkgs installs resolve a safe catalogue identity against the exact consuming-flake revision before using a dedicated Nix profile. The manifest is persisted only after a successful profile transaction and the UI exposes search and Install. Rich detail/category views, Store-owned Nix removal, generation retention policy, desktop reconciliation proof and live Flathub UI remain incomplete.

Store is a standalone sibling project. Vesper owns Settings -> Apps and consumes
the published Store package as an external flake; it does not own Store source
or this architecture document.

The application name is exactly `Vesper Store`.

## product boundary

Vesper Store is a separate native desktop application for discovering and installing applications.

It owns:

- search
- categories
- application discovery
- pre-install details
- source selection
- install planning
- install transactions
- optional Flathub source configuration
- the shared Store transaction core used for Store-owned removals

`Settings -> Apps` owns installed application management:

- installed application list/detail
- Open
- Remove when the real owner supports it
- installed version/size/source state
- permissions
- wellbeing
- optional Liquid Glass icon controls

Do not build a second installed-app management surface inside Vesper Store.

The Store is not a generic nixpkgs browser, NixOS option editor, service manager or frontend for every derivation in nixpkgs.

Normal discovery should contain user-facing desktop applications rather than libraries, headers, runtimes, kernels, drivers, package sets or hidden helpers.

## source policy

Default source priority:

1. Nixpkgs from the same locked revision used by Vesper
2. reviewed Vesper integration recipes when a package alone is insufficient
3. Flathub only after explicit user opt-in

Flathub is optional and disabled by default.

There is no apt, rpm, pacman or PackageKit backend.
There is no package-format conversion layer.

If the Store is ever reused outside NixOS, Nix remains the application layer instead of translating packages into the host distribution format.

## native application contract

Target identity:

```text
name        Vesper Store
app id      io.vesper.Store
desktop id  io.vesper.Store.desktop
binary      vesper-store
```

Target stack:

```text
Qt 6
Qt Quick / QML
Rust backend
SQLite
Nix CLI / established Nix interfaces
```

Qt/QML is the standard Store presentation stack.
GTK/libadwaita is not globally forbidden in Vesper, but it is not the Store UI stack.

Do not implement Store with Electron, Tauri/WebView, an embedded browser, a localhost web frontend or a Caelestia-only page.

QML is presentation logic.
Do not put Nix parsing, Flatpak parsing, SQL construction or transaction state machines into QML JavaScript.

## upstream-first rule

Reuse established upstream infrastructure instead of rebuilding package-management primitives.

Relevant building blocks include:

- Nix itself for dependency resolution, realization, substituters and build failures
- NixOS Search / `flake-info` concepts for package indexing
- NixOS AppStream data for desktop metadata where appropriate
- `nix-software-center` as prior art for Nix GUI-store edge cases
- Flatpak's own CLI, remotes and AppStream mechanisms when optional Flathub support is enabled

Do not scrape flathub.org for core behavior.
Do not create a second resolver, downloader or binary-cache protocol.

## relationship with Settings -> Apps

Flow:

```text
Settings -> Apps
  -> Find New Apps
  -> Vesper Store
  -> Install
  -> desktop-entry reconciliation
  -> Settings -> Apps manages installed app
```

`Find New Apps` launches:

```text
vesper-store
```

Prefer single-instance activation.

Do not add `Open in Vesper Store` to installed application details.
Once installed, Settings -> Apps is the canonical management surface.

## shared application identity

Store and Apps must share one compatible identity model.

Preferred keys:

1. AppStream component ID
2. desktop file ID
3. Nix package attribute when known
4. Flatpak application ID when relevant
5. reviewed aliases only for known mismatches

Do not fuzzy-merge applications merely because names look similar.

A successful install must become visible through real desktop-entry reconciliation.
Do not create a separate incompatible installed-app registry.

## Store UX

Recommended navigation:

```text
Vesper Store
├── Discover
├── Categories
├── Search
└── Sources
```

Do not add a full `Installed` manager.
Global system updates stay under `System -> Updates`.

Result rows/tiles should stay compact:

- catalogue icon
- name
- short summary
- source only when relevant
- installed state when known
- one primary action

Pre-install detail may show:

- catalogue icon
- name
- summary/description
- screenshots
- source
- available version
- homepage
- license
- expected sandbox type
- real local-build warning from the Nix plan
- package attribute under advanced information
- Install

If already installed, show a quiet Installed state rather than duplicating Open/Remove/permissions/icon controls.

Keyboard interaction should include search focus, result navigation, Enter to open detail and Escape to close/back out.

Do not invent install percentages.
Use real phases such as:

```text
planning
resolving
downloading
building
installing
reconciling
complete
failed
```

## visual contract

Store follows the consumer-agnostic Liquid Glass contract from the sibling
`../../liquid-glass/` project only when a consumer explicitly adopts it. It must
not turn the complete application into a transparent shell overlay.

Use:

- calm palette-aware surfaces
- selective translucency where technically appropriate
- concentric rounded geometry
- soft shadow
- restrained borders/highlights
- clear hierarchy and spacing

Avoid:

- neon source colors
- thick glowing borders
- dense telemetry-card layouts
- browser-like store chrome
- dozens of separately blurred tiles

Use a small semantic Qt/QML theme adapter rather than copying arbitrary Caelestia constants.

## catalogue architecture

Nixpkgs browsing should be local and fast.

Target shape:

```text
locked nixpkgs
  + package metadata / flake-info
  + NixOS AppStream data
  + reviewed Vesper overrides
        -> catalogue builder
        -> normalized SQLite + FTS5
        -> Vesper Store
```

The catalogue is tied to Vesper's locked nixpkgs revision and target system.

Suggested derivation output:

```text
/nix/store/...-vesper-store-catalog/
└── share/vesper/store/
    ├── catalog.sqlite
    ├── icons/
    └── catalog-meta.json
```

`catalog-meta.json` should include at least schema version, system, nixpkgs revision and generation timestamp.

The current readiness command reads `VESPER_STORE_CATALOG` and checks the
sidecar at the same directory's `catalog-meta.json`. Set
`VESPER_STORE_CATALOG_META` when the metadata lives elsewhere. The packaged
Store also sets `VESPER_STORE_EXPECTED_SYSTEM` from its target platform and
`VESPER_STORE_EXPECTED_NIXPKGS_REVISION` from the flake lock before the backend
validates the sidecar.

Do not evaluate all nixpkgs on each query.
Do not call a remote search service on every keystroke.
Do not store large screenshot blobs inside SQLite.

## metadata authority

Nixpkgs is authoritative for package facts such as:

- attribute path
- pname/version
- platform support
- broken/insecure state
- license
- homepage
- main program when declared

AppStream enriches presentation with display identity, descriptions, categories, keywords, screenshots and icon metadata.

AppStream must not override the package version Nix will actually install.

## catalogue eligibility

Normal results require strong desktop-app identity, such as an AppStream desktop component or visible `Type=Application` desktop entry.

Hide by default:

- broken packages
- unsupported platforms
- libraries/dev outputs
- language package sets
- kernels/drivers
- non-desktop services
- Flatpak runtimes
- `NoDisplay=true` helpers
- duplicate outputs for one application

If Nixpkgs marks a package insecure, block ordinary one-click installation by default and show the reason.

## search

Use SQLite FTS5 over fields such as:

- display name
- generic name
- aliases
- keywords
- package attribute
- summary

Ranking should prioritize exact name/alias matches before broader summary/description matches.

No AI provider is required for Store search.

The backend command is `vesper-store-core search QUERY`. It validates
the local catalogue before querying, limits input and result count, and returns
machine-readable JSON. The Qt/QML shell consumes that command after a short
debounce and renders compact result rows. `vesper-store --search QUERY` opens
that Search page with an initial query for launcher/deeplink handoff; detail,
install handoff is available through `vesper-store-core nix-install APP_ID`;
rich detail and reconciliation flows remain separate work.

## Nix install core

Do not rewrite `home/yargc/apps.nix` when Install is pressed.
Do not make Store actions rebuild unrelated dirty work in `/home/yargc/nix-config`.

Use a dedicated manifest-driven Nix profile for Store-owned applications.

Suggested state:

```text
~/.config/vesper/store/manifest.json
~/.local/state/vesper/store/profile
~/.local/state/vesper/store/generations.json
~/.local/state/vesper/store/transactions/
~/.local/state/vesper/store/gcroots/
~/.cache/vesper/store/media/
$XDG_RUNTIME_DIR/vesper/store.lock
```

The Rust Store backend exposes this path contract through
`vesper-store-core store-state` and creates the private state directories and
an empty manifest with `vesper-store-core store-init`. `manifest-status` rejects
unknown schema,
duplicate application ids, unsafe identifiers and source entries without the
required package or Flatpak identifier.

The profile's `bin` and `share` directories are the session handoff paths. The
profile and retained generation links must still be registered as explicit Nix
GC roots by the transaction implementation; creating a local `gcroots`
directory alone is not a GC-root proof.

The manifest is authoritative only for Store-owned selections.
Existing packages declared by the Vesper configuration remain separate and must not be silently migrated.

Do not store arbitrary executable Nix expressions in mutable or remote Store metadata.

## pinning

Store installs must use the same exact nixpkgs revision as the Store/catalogue build.

Keep these coherent:

```text
Vesper system revision
Vesper Store catalogue revision
Vesper Store package revision
```

Do not let `nixpkgs#foo` silently follow a newer registry revision than Vesper.

## transaction model

All package mutations are serialized even when initiated from Settings.

Target Nix install flow:

1. acquire Store lock
2. validate current manifest
3. calculate desired manifest
4. resolve against pinned nixpkgs
5. request a Nix dry-run plan
6. show policy/local-build warnings
7. realize desired packages
8. atomically switch Store profile
9. atomically persist manifest
10. reconcile desktop application identity
11. make the resulting desktop entry available to normal Apps inspection
12. release lock

A failed realization leaves the previous profile and manifest active.

Never persist desired state first and hope the build succeeds later.

Removal of a Store-owned app uses the same core with that app removed from desired state.

## rollback and GC

Keep several successful Store generations and manifest snapshots.
Rollback restores a known-good Store generation as a whole.

Active and retained Store generations must remain GC roots.
Verify normal `nh clean` does not remove active Store applications.

## installed size handoff

Installed size belongs to `Settings -> Apps`, not speculative Store catalogue badges.

For Store-managed Nix applications, use realized Nix closure/store information.
For Flatpak, use Flatpak installed/deployed size.
Show unknown when a trustworthy value is unavailable.

## optional Flathub

Flathub support is disabled by default and requires explicit user opt-in.

When enabled:

- use Flatpak's real remotes and AppStream metadata
- keep source identity explicit
- keep Flatpak permissions in Settings -> Apps
- do not silently prefer Flatpak over an available Nixpkgs application without a documented source-selection rule

Disabling Flathub must not corrupt already-installed application state.

## security and trust

- never add third-party Nix caches automatically
- never execute arbitrary command text from catalogue metadata
- keep package/source identity explicit
- preserve Nixpkgs broken/insecure policy information
- keep Flatpak remote changes explicit
- keep Store state local
- do not store provider/API secrets in Store state

## implementation rule

When implementing this specification:

1. inspect current Store code first
2. preserve the Qt/QML + Rust boundary
3. reuse Nix/Flatpak/AppStream infrastructure
4. keep package state transactional and reversible
5. keep installed-app management in Settings -> Apps
6. keep optional Liquid Glass integration ownership in `ADAPTIVE-ICONS.md`
7. update the `current implementation` statement at the top as milestones land
8. do not describe target behavior as already implemented
