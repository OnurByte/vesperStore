{
  lib,
  pkg-config,
  qt6,
  jq,
  gzip,
  nixosAppstreamData,
  nixpkgsRevision ? null,
  rustc,
  sqlite,
  stdenv,
  upstreamRevision,
  upstreamSource,
  yq-go,
}:
let
  expectedRevision = if nixpkgsRevision == null then "" else nixpkgsRevision;
  fixtureRevision =
    if expectedRevision == "" then "0000000000000000000000000000000000000000" else expectedRevision;
  expectedRevisionArg = lib.optionalString (nixpkgsRevision != null)
    "--set VESPER_STORE_EXPECTED_NIXPKGS_REVISION ${lib.escapeShellArg nixpkgsRevision}";
in
stdenv.mkDerivation {
  pname = "vesper-store";
  version = "0.1.0";

  src = ./.;

  nativeBuildInputs = [
    jq
    gzip
    pkg-config
    qt6.wrapQtAppsHook
    rustc
    sqlite
    yq-go
  ];

  buildInputs = [
    qt6.qtbase
    qt6.qtdeclarative
    qt6.qtwayland
  ];

  buildPhase = ''
    runHook preBuild

    $CXX -std=c++20 -O2 -fPIC \
      src/main.cpp \
      -o vesper-store \
      $(pkg-config --cflags --libs Qt6Core Qt6Gui Qt6Qml Qt6Quick)

    rustc --edition=2021 -C opt-level=2 \
      src/backend.rs \
      -o vesper-store-core

    runHook postBuild
  '';

  doCheck = true;
  checkPhase = ''
    runHook preCheck

    ./vesper-store-core sources \
      | grep -F '"flathub":{"enabled":false' >/dev/null
    rustc --edition=2021 --test src/backend.rs -o vesper-store-core-tests
    ./vesper-store-core-tests
    env -u VESPER_STORE_CATALOG ./vesper-store-core catalog-status \
      | grep -F '"available":false' >/dev/null

    fixture="$TMPDIR/catalog.sqlite"
    sqlite3 "$fixture" < data/catalog-schema.sql
    printf '%s\n' '{"schemaVersion":1,"system":"${stdenv.hostPlatform.system}","nixpkgsRevision":"${fixtureRevision}","generatedAt":"2026-01-01T00:00:00Z"}' \
      > "$TMPDIR/catalog-meta.json"
    VESPER_STORE_EXPECTED_SYSTEM="${stdenv.hostPlatform.system}" \
      VESPER_STORE_EXPECTED_NIXPKGS_REVISION="${expectedRevision}" \
      VESPER_STORE_CATALOG="$fixture" ./vesper-store-core catalog-status \
      | grep -F '"available":true' >/dev/null
    sqlite3 "$fixture" <<'SQL'
    INSERT INTO apps (id, name, summary, primary_category)
      VALUES ('org.example.Browser', 'Example Browser', 'A private web browser', 'Network');
    INSERT INTO variants (app_id, source_kind, source_id, package_attr, package_version, sandbox_kind, install_kind)
      VALUES ('org.example.Browser', 'nixpkgs', 'example-browser', 'example-browser', '1.0', 'native', 'profile');
    INSERT INTO apps (id, name, summary, primary_category)
      VALUES ('org.example.Editor', 'Example Editor', 'A text editor', 'Utility');
    INSERT INTO variants (app_id, source_kind, source_id, package_attr, package_version, sandbox_kind, install_kind)
      VALUES ('org.example.Editor', 'nixpkgs', 'example-editor', 'example-editor', '1.0', 'native', 'profile');
    INSERT INTO apps_fts (app_id, name, generic_name, aliases, keywords, package_attr, summary, description)
      VALUES ('org.example.Browser', 'Example Browser', 'Web Browser', 'browser', 'internet web', 'example-browser', 'A private web browser', char());
    SQL
    VESPER_STORE_EXPECTED_SYSTEM="${stdenv.hostPlatform.system}" \
      VESPER_STORE_EXPECTED_NIXPKGS_REVISION="${expectedRevision}" \
      VESPER_STORE_CATALOG="$fixture" ./vesper-store-core search browser \
      | grep -F '"id":"org.example.Browser"' >/dev/null

    fake_nix="$TMPDIR/fake-nix"
    printf '%s\n' \
      '#!/bin/sh' \
      'if [ "$1" = build ]; then exit 0; fi' \
      'if [ "$1" = profile ] && [ "$2" = install ]; then test "''${VESPER_FAKE_FAIL:-0}" != 1; exit; fi' \
      'exit 1' > "$fake_nix"
    chmod +x "$fake_nix"

    store_home="$TMPDIR/store-home"
    store_runtime="$TMPDIR/store-runtime"
    HOME="$store_home" XDG_RUNTIME_DIR="$store_runtime" ./vesper-store-core store-init \
      | jq -e '.available == true and .profile.present == false' >/dev/null
    test -f "$store_home/.config/vesper/store/manifest.json"
    test "$(stat -c '%a' "$store_home/.config/vesper/store/manifest.json")" = 600
    HOME="$store_home" XDG_RUNTIME_DIR="$store_runtime" ./vesper-store-core manifest-status \
      | jq -e '.available == true' >/dev/null
    HOME="$store_home" XDG_RUNTIME_DIR="$store_runtime" \
      VESPER_STORE_CATALOG="$fixture" \
      VESPER_STORE_EXPECTED_SYSTEM="${stdenv.hostPlatform.system}" \
      VESPER_STORE_EXPECTED_NIXPKGS_REVISION="${fixtureRevision}" \
      VESPER_STORE_NIX="$fake_nix" \
      ./vesper-store-core nix-install org.example.Browser \
      | jq -e '.available == true and .phase == "complete"' >/dev/null
    jq -e '.apps == [{"id":"org.example.Browser","source":"nixpkgs","packageAttr":"example-browser"}]' \
      "$store_home/.config/vesper/store/manifest.json" >/dev/null
    HOME="$store_home" XDG_RUNTIME_DIR="$store_runtime" VESPER_FAKE_FAIL=1 \
      VESPER_STORE_CATALOG="$fixture" \
      VESPER_STORE_EXPECTED_SYSTEM="${stdenv.hostPlatform.system}" \
      VESPER_STORE_EXPECTED_NIXPKGS_REVISION="${fixtureRevision}" \
      VESPER_STORE_NIX="$fake_nix" \
      ./vesper-store-core nix-install org.example.Editor \
      | jq -e '.available == false and .phase == "failed"' >/dev/null
    jq -e '.apps == [{"id":"org.example.Browser","source":"nixpkgs","packageAttr":"example-browser"}]' \
      "$store_home/.config/vesper/store/manifest.json" >/dev/null
    printf '%s\n' '{"schemaVersion":1,"apps":[{"id":"org.example.Browser","source":"nixpkgs","packageAttr":"example-browser"},{"id":"org.example.Browser","source":"nixpkgs","packageAttr":"example-browser"}]}' \
      > "$TMPDIR/invalid-manifest.json"
    VESPER_STORE_MANIFEST="$TMPDIR/invalid-manifest.json" ./vesper-store-core manifest-status \
      | jq -e '.available == false' >/dev/null
    printf '%s\n' '{"schemaVersion":1,"apps":[{"id":"org.example.Browser","source":"nixpkgs","packageAttr":"example-browser;touch"}]}' \
      > "$TMPDIR/unsafe-manifest.json"
    VESPER_STORE_MANIFEST="$TMPDIR/unsafe-manifest.json" ./vesper-store-core manifest-status \
      | jq -e '.available == false' >/dev/null

    incomplete="$TMPDIR/catalog-incomplete.sqlite"
    sqlite3 "$incomplete" < data/catalog-schema.sql
    sqlite3 "$incomplete" 'DROP TABLE aliases;'
    VESPER_STORE_EXPECTED_SYSTEM="${stdenv.hostPlatform.system}" \
      VESPER_STORE_EXPECTED_NIXPKGS_REVISION="${expectedRevision}" \
      VESPER_STORE_CATALOG="$incomplete" VESPER_STORE_CATALOG_META="$TMPDIR/catalog-meta.json" \
      ./vesper-store-core catalog-status | grep -F '"available":false' >/dev/null

    invalid_meta="$TMPDIR/catalog-meta-invalid.json"
    printf '%s\n' '{"schemaVersion":1,"system":"x86_64-linux","nixpkgsRevision":"not-a-revision","generatedAt":"2026-01-01T00:00:00Z"}' \
      > "$invalid_meta"
    VESPER_STORE_EXPECTED_SYSTEM="${stdenv.hostPlatform.system}" \
      VESPER_STORE_EXPECTED_NIXPKGS_REVISION="${expectedRevision}" \
      VESPER_STORE_CATALOG="$fixture" VESPER_STORE_CATALOG_META="$invalid_meta" \
      ./vesper-store-core catalog-status | grep -F '"available":false' >/dev/null

    runHook postCheck
  '';

  installPhase = ''
    runHook preInstall

    install -Dm755 vesper-store "$out/bin/vesper-store"
    install -Dm755 vesper-store-core "$out/libexec/vesper-store-core"
    install -Dm644 qml/Main.qml "$out/share/vesper-store/qml/Main.qml"
    install -Dm644 data/catalog-schema.sql "$out/share/vesper-store/catalog-schema.sql"
    install -Dm644 data/io.vesper.Store.desktop "$out/share/applications/io.vesper.Store.desktop"
    install -Dm644 data/io.vesper.Store.metainfo.xml "$out/share/metainfo/io.vesper.Store.metainfo.xml"
    install -Dm644 ${upstreamSource}/LICENSE "$out/share/licenses/vesper-store/nix-software-center-LICENSE"

    catalogDir="$out/share/vesper/store"
    mkdir -p "$catalogDir"
    gzip -dc ${nixosAppstreamData}/share/app-info/xmls/nixos_${lib.replaceStrings [ "-" ] [ "_" ] stdenv.hostPlatform.system}.yml.gz \
      | yq -o=json -I=0 '.' \
      | jq -s '[.[] | select(
          .Type == "desktop-application"
          and (.ID | type == "string")
          and (.ID | test("^[A-Za-z0-9][A-Za-z0-9._-]*$"))
          and (.Package | type == "string")
          and (.Package | test("^[A-Za-z0-9][A-Za-z0-9._-]*$"))
        )]' > "$TMPDIR/appstream.json"
    sqlite3 "$catalogDir/catalog.sqlite" < data/catalog-schema.sql
    sqlite3 "$catalogDir/catalog.sqlite" <<SQL
    INSERT OR IGNORE INTO apps (
      id, name, generic_name, summary, description, appstream_id,
      desktop_id, homepage, icon_key, primary_category
    )
    SELECT
      json_extract(value, '$.ID'),
      COALESCE(json_extract(value, '$.Name.C'), json_extract(value, '$.Name.en'), json_extract(value, '$.Package')),
      char(),
      COALESCE(json_extract(value, '$.Summary.C'), json_extract(value, '$.Summary.en'), char()),
      COALESCE(json_extract(value, '$.Description.C'), json_extract(value, '$.Description.en'), char()),
      json_extract(value, '$.ID'),
      json_extract(value, '$.Launchable."desktop-id"[0]'),
      json_extract(value, '$.Url.homepage'),
      COALESCE(json_extract(value, '$.Icon.stock'), json_extract(value, '$.Icon.cached[0].name')),
      json_extract(value, '$.Categories[0]')
    FROM json_each(readfile('$TMPDIR/appstream.json'));

    INSERT OR IGNORE INTO variants (
      app_id, source_kind, source_id, package_attr, package_version,
      sandbox_kind, install_kind, supported, broken, insecure
    )
    SELECT
      json_extract(value, '$.ID'), 'nixpkgs', json_extract(value, '$.Package'),
      json_extract(value, '$.Package'), char(), 'native', 'profile', 1, 0, 0
    FROM json_each(readfile('$TMPDIR/appstream.json'))
    WHERE EXISTS (SELECT 1 FROM apps WHERE apps.id = json_extract(value, '$.ID'));

    INSERT INTO apps_fts (
      app_id, name, generic_name, aliases, keywords, package_attr, summary, description
    )
    SELECT apps.id, apps.name, apps.generic_name, char(), char(), variants.package_attr, apps.summary, apps.description
    FROM apps JOIN variants ON variants.app_id = apps.id;
SQL
    printf '%s\n' \
      '{"schemaVersion":1,"system":"${stdenv.hostPlatform.system}","nixpkgsRevision":"${fixtureRevision}","generatedAt":"2024-04-11T00:00:00Z","upstream":{"repository":"snowfallorg/nix-software-center","revision":"${upstreamRevision}"}}' \
      > "$catalogDir/catalog-meta.json"

    runHook postInstall
  '';

  preFixup = ''
    qtWrapperArgs+=(
      --set VESPER_STORE_QML "$out/share/vesper-store/qml/Main.qml"
      --set VESPER_STORE_CORE "$out/libexec/vesper-store-core"
      --set VESPER_STORE_EXPECTED_SYSTEM "${stdenv.hostPlatform.system}"
      --set VESPER_STORE_CATALOG "$out/share/vesper/store/catalog.sqlite"
      ${expectedRevisionArg}
      --prefix PATH : "${lib.makeBinPath [ jq sqlite ]}"
    )
  '';

  doInstallCheck = true;
  installCheckPhase = ''
    VESPER_STORE_CATALOG="$out/share/vesper/store/catalog.sqlite" \
      VESPER_STORE_EXPECTED_SYSTEM="${stdenv.hostPlatform.system}" \
      VESPER_STORE_EXPECTED_NIXPKGS_REVISION="${expectedRevision}" \
      "$out/libexec/vesper-store-core" catalog-status \
      | jq -e '.available == true' >/dev/null
  '';

  meta = {
    description = "Native Qt/QML application discovery surface for Vesper";
    license = lib.licenses.mit;
    mainProgram = "vesper-store";
    platforms = lib.platforms.linux;
  };
}
