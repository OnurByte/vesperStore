use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const CATALOG_SCHEMA_VERSION: u32 = 1;
const REQUIRED_TABLES: u32 = 8;
const REQUIRED_COLUMNS: u32 = 44;
const SEARCH_LIMIT: u32 = 24;
const STORE_STATE_SCHEMA_VERSION: u32 = 1;
const STORE_SOURCES_SCHEMA_VERSION: u32 = 1;
const FLATHUB_REMOTE: &str = "flathub";
const FLATHUB_REPO: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

const CATALOG_SCHEMA_QUERY: &str = r#"
PRAGMA user_version;
WITH required_tables(name) AS (
    VALUES
        ('apps'),
        ('variants'),
        ('categories'),
        ('app_categories'),
        ('screenshots'),
        ('keywords'),
        ('aliases'),
        ('apps_fts')
), required_columns(table_name, column_name) AS (
    VALUES
        ('apps', 'id'), ('apps', 'name'), ('apps', 'generic_name'),
        ('apps', 'summary'), ('apps', 'description'), ('apps', 'appstream_id'),
        ('apps', 'desktop_id'), ('apps', 'homepage'), ('apps', 'icon_key'),
        ('apps', 'primary_category'),
        ('variants', 'id'), ('variants', 'app_id'), ('variants', 'source_kind'),
        ('variants', 'source_id'), ('variants', 'package_attr'),
        ('variants', 'package_version'), ('variants', 'flatpak_id'),
        ('variants', 'license'), ('variants', 'sandbox_kind'),
        ('variants', 'install_kind'), ('variants', 'supported'),
        ('variants', 'broken'), ('variants', 'insecure'),
        ('categories', 'id'), ('categories', 'name'),
        ('app_categories', 'app_id'), ('app_categories', 'category_id'),
        ('screenshots', 'id'), ('screenshots', 'app_id'), ('screenshots', 'url'),
        ('screenshots', 'caption'), ('screenshots', 'position'),
        ('keywords', 'app_id'), ('keywords', 'keyword'),
        ('aliases', 'app_id'), ('aliases', 'alias'),
        ('apps_fts', 'app_id'), ('apps_fts', 'name'),
        ('apps_fts', 'generic_name'), ('apps_fts', 'aliases'),
        ('apps_fts', 'keywords'), ('apps_fts', 'package_attr'),
        ('apps_fts', 'summary'), ('apps_fts', 'description')
)
SELECT
    (SELECT count(*) FROM required_tables r
     WHERE EXISTS (
         SELECT 1 FROM sqlite_master m
         WHERE m.type = 'table' AND m.name = r.name
     )),
    (SELECT count(*) FROM required_columns c
     WHERE EXISTS (
         SELECT 1 FROM pragma_table_info(c.table_name) p
         WHERE p.name = c.column_name
     )),
    (SELECT count(*) FROM sqlite_master
     WHERE type = 'table' AND name = 'apps_fts'
       AND lower(sql) LIKE '%using fts5%');
"#;

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn catalog_path() -> Option<PathBuf> {
    env::var_os("VESPER_STORE_CATALOG")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

struct StorePaths {
    manifest: PathBuf,
    sources: PathBuf,
    state: PathBuf,
    profile: PathBuf,
    generations: PathBuf,
    transactions: PathBuf,
    media: PathBuf,
    gc_roots: PathBuf,
    lock: Option<PathBuf>,
}

fn xdg_path(variable: &str, home: &Path, fallback: &str) -> PathBuf {
    env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(fallback))
}

fn store_paths() -> Result<StorePaths, String> {
    let home = env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not configured".to_string())?;
    let config = xdg_path("XDG_CONFIG_HOME", &home, ".config");
    let state = xdg_path("XDG_STATE_HOME", &home, ".local/state").join("vesper/store");
    let cache = xdg_path("XDG_CACHE_HOME", &home, ".cache").join("vesper/store");
    let runtime = env::var_os("XDG_RUNTIME_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|path| path.join("vesper/store.lock"));

    Ok(StorePaths {
        manifest: config.join("vesper/store/manifest.json"),
        sources: config.join("vesper/store/sources.json"),
        profile: state.join("profile"),
        generations: state.join("generations.json"),
        transactions: state.join("transactions"),
        media: cache.join("media"),
        gc_roots: state.join("gcroots"),
        state,
        lock: runtime,
    })
}

fn manifest_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("VESPER_STORE_MANIFEST").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    Ok(store_paths()?.manifest)
}

fn json_path(path: &Path) -> String {
    json_escape(&path.to_string_lossy())
}

fn json_optional_path(path: Option<&Path>) -> String {
    path.map(json_path)
        .map(|value| format!("\"{value}\""))
        .unwrap_or_else(|| "null".to_string())
}

fn path_entry_count(path: &Path) -> usize {
    fs::read_dir(path)
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0)
}

fn print_store_state() {
    let paths = match store_paths() {
        Ok(paths) => paths,
        Err(error) => {
            println!(
                "{{\"schemaVersion\":{STORE_STATE_SCHEMA_VERSION},\"available\":false,\"error\":\"{}\"}}",
                json_escape(&error)
            );
            return;
        }
    };
    let profile_target = fs::read_link(&paths.profile)
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let profile_present = fs::symlink_metadata(&paths.profile).is_ok();
    println!(
        "{{\"schemaVersion\":{STORE_STATE_SCHEMA_VERSION},\"available\":true,\"paths\":{{\"manifest\":\"{}\",\"state\":\"{}\",\"profile\":\"{}\",\"generations\":\"{}\",\"transactions\":\"{}\",\"media\":\"{}\",\"gcRoots\":\"{}\",\"lock\":{}}},\"profile\":{{\"present\":{},\"target\":\"{}\"}},\"gcRoots\":{{\"entries\":{}}},\"session\":{{\"bin\":\"{}\",\"data\":\"{}\"}}}}",
        json_path(&paths.manifest),
        json_path(&paths.state),
        json_path(&paths.profile),
        json_path(&paths.generations),
        json_path(&paths.transactions),
        json_path(&paths.media),
        json_path(&paths.gc_roots),
        json_optional_path(paths.lock.as_deref()),
        profile_present,
        json_escape(&profile_target),
        path_entry_count(&paths.gc_roots),
        json_path(&paths.profile.join("bin")),
        json_path(&paths.profile.join("share")),
    );
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| format!("could not secure {}: {error}", path.display()))
}

fn initialize_store_state() -> Result<(), String> {
    let paths = store_paths()?;
    let config_store = paths
        .manifest
        .parent()
        .ok_or_else(|| "manifest parent is missing".to_string())?;
    for directory in [
        config_store,
        paths.state.as_path(),
        paths.transactions.as_path(),
        paths.media.as_path(),
        paths.gc_roots.as_path(),
    ] {
        fs::create_dir_all(directory)
            .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
        #[cfg(unix)]
        set_mode(directory, 0o700)?;
    }

    if let Some(lock) = &paths.lock {
        if let Some(parent) = lock.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
            #[cfg(unix)]
            set_mode(parent, 0o700)?;
        }
    }

    if !paths.manifest.exists() {
        atomic_private_write(&paths.manifest, "{\"schemaVersion\":1,\"apps\":[]}\n")?;
    }
    if !paths.sources.exists() {
        atomic_private_write(
            &paths.sources,
            "{\"schemaVersion\":1,\"flathubEnabled\":false}\n",
        )?;
    }
    Ok(())
}

fn atomic_private_write(path: &Path, contents: &str) -> Result<(), String> {
    let temporary = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("state"),
        std::process::id()
    ));
    fs::write(&temporary, contents)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    #[cfg(unix)]
    set_mode(&temporary, 0o600)?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("could not install {}: {error}", path.display()))
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'.' | b'_' | b'-'))
        })
}

struct StoreLock(PathBuf);

impl StoreLock {
    fn acquire(paths: &StorePaths) -> Result<Self, String> {
        let path = paths
            .lock
            .as_ref()
            .ok_or_else(|| "XDG_RUNTIME_DIR is required for Store transactions".to_string())?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| format!("another Store transaction is active: {error}"))?;
        #[cfg(unix)]
        set_mode(path, 0o600)?;
        writeln!(file, "{}", std::process::id())
            .map_err(|error| format!("could not initialize Store lock: {error}"))?;
        Ok(Self(path.clone()))
    }
}

impl Drop for StoreLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn print_store_init() {
    match initialize_store_state() {
        Ok(()) => print_store_state(),
        Err(error) => println!(
            "{{\"schemaVersion\":{STORE_STATE_SCHEMA_VERSION},\"available\":false,\"error\":\"{}\"}}",
            json_escape(&error)
        ),
    }
}

fn inspect_manifest(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err("manifest file does not exist".to_string());
    }
    let output = Command::new("jq")
        .args([
            "-e",
            r#"
                def safe_identifier:
                    (type == "string" and length > 0 and length <= 256
                     and test("^[A-Za-z0-9][A-Za-z0-9._-]*$"));
                def valid_app:
                    (type == "object")
                    and (.id | safe_identifier)
                    and (
                        (.source == "nixpkgs" and (.packageAttr | safe_identifier))
                        or (.source == "flathub" and (.flatpakId | safe_identifier))
                    );
                if type != "object"
                   or .schemaVersion != 1
                   or (.apps | type) != "array"
                   or any(.apps[]; valid_app | not)
                   or ((.apps | map(.id) | unique | length) != (.apps | length))
                then error("store manifest contract mismatch")
                else true
                end
            "#,
        ])
        .arg(path)
        .output()
        .map_err(|error| format!("could not run jq: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if error.is_empty() {
        "store manifest contract mismatch".to_string()
    } else {
        error
    })
}

fn print_manifest_status() {
    let path = match manifest_path() {
        Ok(path) => path,
        Err(error) => {
            println!(
                "{{\"schemaVersion\":1,\"available\":false,\"path\":\"\",\"error\":\"{}\"}}",
                json_escape(&error)
            );
            return;
        }
    };
    match inspect_manifest(&path) {
        Ok(()) => println!(
            "{{\"schemaVersion\":1,\"available\":true,\"path\":\"{}\",\"error\":\"\"}}",
            json_path(&path)
        ),
        Err(error) => println!(
            "{{\"schemaVersion\":1,\"available\":false,\"path\":\"{}\",\"error\":\"{}\"}}",
            json_path(&path),
            json_escape(&error)
        ),
    }
}

fn catalog_metadata_path(catalog: &Path) -> PathBuf {
    env::var_os("VESPER_STORE_CATALOG_META")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            catalog
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("catalog-meta.json")
        })
}

fn inspect_metadata(path: &Path, schema_version: u32) -> Result<(), String> {
    if !path.is_file() {
        return Err("catalogue metadata file does not exist".to_string());
    }

    let expected_system = env::var("VESPER_STORE_EXPECTED_SYSTEM").unwrap_or_default();
    let expected_revision = env::var("VESPER_STORE_EXPECTED_NIXPKGS_REVISION").unwrap_or_default();
    let output = Command::new("jq")
        .args([
            "-e",
            "--argjson",
            "expectedSchema",
            &schema_version.to_string(),
            "--arg",
            "expectedSystem",
            &expected_system,
            "--arg",
            "expectedRevision",
            &expected_revision,
            r#"
                if type != "object"
                   or (.schemaVersion | type) != "number"
                   or (.schemaVersion != $expectedSchema)
                   or (.system | type) != "string"
                   or (.system == "")
                   or ($expectedSystem != "" and .system != $expectedSystem)
                   or (.nixpkgsRevision | type) != "string"
                   or (.nixpkgsRevision | test("^[0-9a-fA-F]{7,64}$") | not)
                   or ($expectedRevision != "" and .nixpkgsRevision != $expectedRevision)
                   or (.generatedAt | type) != "string"
                   or (.generatedAt == "")
                then error("catalogue metadata contract mismatch")
                else true
                end
            "#,
        ])
        .arg(path)
        .output()
        .map_err(|error| format!("could not run jq: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if error.is_empty() {
        "catalogue metadata contract mismatch".to_string()
    } else {
        error
    })
}

fn inspect_catalog(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err("catalogue file does not exist".to_string());
    }

    let canonical = path
        .canonicalize()
        .map_err(|error| format!("could not resolve catalogue path: {error}"))?;
    let output = Command::new("sqlite3")
        .args(["-readonly", "-batch", "-noheader"])
        .arg(&canonical)
        .arg(CATALOG_SCHEMA_QUERY)
        .output()
        .map_err(|error| format!("could not run sqlite3: {error}"))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if error.is_empty() {
            "sqlite3 rejected catalogue".to_string()
        } else {
            error
        });
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| "sqlite3 returned non-UTF-8 output".to_string())?;
    let mut lines = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let version = lines
        .next()
        .ok_or_else(|| "catalogue schema version is missing".to_string())?
        .parse::<u32>()
        .map_err(|_| "catalogue schema version is invalid".to_string())?;
    let inventory = lines
        .next()
        .ok_or_else(|| "catalogue table inventory is missing".to_string())?
        .split('|')
        .map(str::trim)
        .map(|value| value.parse::<u32>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "catalogue table inventory is invalid".to_string())?;

    if version != CATALOG_SCHEMA_VERSION {
        return Err(format!(
            "catalogue schema mismatch: expected {CATALOG_SCHEMA_VERSION}, got {version}"
        ));
    }
    if inventory.len() != 3 {
        return Err("catalogue schema inventory is incomplete".to_string());
    }
    if inventory[0] != REQUIRED_TABLES {
        return Err("catalogue is missing required tables".to_string());
    }
    if inventory[1] != REQUIRED_COLUMNS {
        return Err("catalogue is missing required columns".to_string());
    }
    if inventory[2] != 1 {
        return Err("catalogue FTS table is missing or invalid".to_string());
    }

    inspect_metadata(&catalog_metadata_path(&canonical), version)
}

fn normalized_search_terms(query: &str) -> Result<Vec<String>, String> {
    if query.chars().count() > 128 {
        return Err("search query is too long".to_string());
    }

    let mut terms = Vec::new();
    let mut current = String::new();
    for character in query.chars() {
        if character.is_alphanumeric() || matches!(character, '_' | '-' | '.') {
            current.push(character);
        } else if !current.is_empty() {
            terms.push(std::mem::take(&mut current));
        }
        if terms.len() == 8 {
            break;
        }
    }
    if !current.is_empty() && terms.len() < 8 {
        terms.push(current);
    }

    for term in &mut terms {
        *term = term.chars().take(48).collect();
    }
    terms.retain(|term| !term.is_empty());
    Ok(terms)
}

fn sql_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn nix_program() -> PathBuf {
    env::var_os("VESPER_STORE_NIX")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("nix"))
}

fn nixpkgs_revision() -> Result<String, String> {
    let revision = env::var("VESPER_STORE_EXPECTED_NIXPKGS_REVISION")
        .map_err(|_| "locked Nixpkgs revision is not configured".to_string())?;
    if revision.len() < 7
        || revision.len() > 64
        || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("locked Nixpkgs revision is invalid".to_string());
    }
    Ok(revision)
}

fn nix_install_target(path: &Path, app_id: &str) -> Result<String, String> {
    inspect_catalog(path)?;
    if !safe_identifier(app_id) {
        return Err("invalid application id".to_string());
    }
    let sql = format!(
        "SELECT package_attr FROM variants \
         WHERE app_id = {} AND source_kind = 'nixpkgs' \
           AND supported = 1 AND broken = 0 AND insecure = 0 \
           AND package_attr IS NOT NULL \
         ORDER BY id LIMIT 1;",
        sql_literal(app_id)
    );
    let output = Command::new("sqlite3")
        .args(["-readonly", "-batch", "-noheader"])
        .arg(path)
        .arg(sql)
        .output()
        .map_err(|error| format!("could not query catalogue: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let package_attr = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !safe_identifier(&package_attr) {
        return Err("application has no safe installable Nixpkgs variant".to_string());
    }
    Ok(package_attr)
}

fn manifest_with_nix_package(
    path: &Path,
    app_id: &str,
    package_attr: &str,
) -> Result<String, String> {
    inspect_manifest(path)?;
    let output = Command::new("jq")
        .args([
            "--arg",
            "appId",
            app_id,
            "--arg",
            "packageAttr",
            package_attr,
            r#".apps = ([.apps[] | select(.id != $appId)] + [{id:$appId,source:"nixpkgs",packageAttr:$packageAttr}])"#,
        ])
        .arg(path)
        .output()
        .map_err(|error| format!("could not update Store manifest: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    String::from_utf8(output.stdout).map_err(|_| "jq returned a non-UTF-8 manifest".to_string())
}

fn install_nix_package(app_id: &str) -> Result<String, String> {
    initialize_store_state()?;
    let paths = store_paths()?;
    let catalog = catalog_path().ok_or_else(|| "catalogue path not configured".to_string())?;
    let package_attr = nix_install_target(&catalog, app_id)?;
    let revision = nixpkgs_revision()?;
    let installable = format!("github:NixOS/nixpkgs/{revision}#{package_attr}");
    let desired_manifest = manifest_with_nix_package(&paths.manifest, app_id, &package_attr)?;
    let _lock = StoreLock::acquire(&paths)?;

    let plan = Command::new(nix_program())
        .args(["build", "--dry-run", "--no-link"])
        .arg(&installable)
        .output()
        .map_err(|error| format!("could not plan Nix installation: {error}"))?;
    if !plan.status.success() {
        return Err(format!(
            "Nix installation plan failed: {}",
            String::from_utf8_lossy(&plan.stderr).trim()
        ));
    }

    let install = Command::new(nix_program())
        .args(["profile", "install", "--profile"])
        .arg(&paths.profile)
        .arg(&installable)
        .output()
        .map_err(|error| format!("could not run Nix installation: {error}"))?;
    if !install.status.success() {
        return Err(format!(
            "Nix installation failed: {}",
            String::from_utf8_lossy(&install.stderr).trim()
        ));
    }

    if let Err(error) = atomic_private_write(&paths.manifest, &desired_manifest) {
        let rollback = Command::new(nix_program())
            .args(["profile", "rollback", "--profile"])
            .arg(&paths.profile)
            .status();
        return Err(match rollback {
            Ok(status) if status.success() => error,
            _ => format!("{error}; Nix profile rollback also failed"),
        });
    }
    Ok(package_attr)
}

fn print_install_nix(app_id: Option<String>) {
    let Some(app_id) = app_id else {
        usage();
    };
    match install_nix_package(&app_id) {
        Ok(package_attr) => println!(
            "{{\"schemaVersion\":1,\"available\":true,\"phase\":\"complete\",\"source\":\"nixpkgs\",\"id\":\"{}\",\"packageAttr\":\"{}\"}}",
            json_escape(&app_id),
            json_escape(&package_attr)
        ),
        Err(error) => println!(
            "{{\"schemaVersion\":1,\"available\":false,\"phase\":\"failed\",\"source\":\"nixpkgs\",\"id\":\"{}\",\"error\":\"{}\"}}",
            json_escape(&app_id),
            json_escape(&error)
        ),
    }
}

fn search_catalog(path: &Path, query: &str) -> Result<String, String> {
    inspect_catalog(path)?;
    let terms = normalized_search_terms(query)?;
    if terms.is_empty() {
        return Ok("[]".to_string());
    }

    let fts_query = terms
        .iter()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ");
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("could not resolve catalogue path: {error}"))?;
    let sql = format!(
        "SELECT apps.id AS id, apps.name AS name, apps.summary AS summary, \
                COALESCE((SELECT variants.source_kind FROM variants \
                          WHERE variants.app_id = apps.id ORDER BY variants.id LIMIT 1), '') AS source, \
                COALESCE((SELECT variants.package_attr FROM variants \
                          WHERE variants.app_id = apps.id ORDER BY variants.id LIMIT 1), '') AS packageAttr \
         FROM apps_fts \
         JOIN apps ON apps.id = apps_fts.app_id \
         WHERE apps_fts MATCH {} \
         ORDER BY bm25(apps_fts, 0.0, 10.0, 4.0, 9.0, 7.0, 6.0, 2.0, 1.0), apps.name \
         LIMIT {SEARCH_LIMIT};",
        sql_literal(&fts_query)
    );
    let output = Command::new("sqlite3")
        .args(["-readonly", "-batch", "-json"])
        .arg(canonical)
        .arg(sql)
        .output()
        .map_err(|error| format!("could not run sqlite3: {error}"))?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if error.is_empty() {
            "catalogue search failed".to_string()
        } else {
            error
        });
    }

    let result = String::from_utf8(output.stdout)
        .map_err(|_| "catalogue search returned non-UTF-8 output".to_string())?;
    let result = result.trim();
    if result.is_empty() {
        return Ok("[]".to_string());
    }
    Ok(result.to_string())
}

fn print_catalog_status() {
    match catalog_path() {
        Some(path) => {
            let display = json_escape(&path.to_string_lossy());
            match inspect_catalog(&path) {
                Ok(()) => println!(
                    "{{\"schemaVersion\":{CATALOG_SCHEMA_VERSION},\"available\":true,\"path\":\"{display}\",\"error\":\"\"}}"
                ),
                Err(error) => println!(
                    "{{\"schemaVersion\":{CATALOG_SCHEMA_VERSION},\"available\":false,\"path\":\"{display}\",\"error\":\"{}\"}}",
                    json_escape(&error)
                ),
            }
        }
        None => println!(
            "{{\"schemaVersion\":{CATALOG_SCHEMA_VERSION},\"available\":false,\"path\":\"\",\"error\":\"catalogue path not configured\"}}"
        ),
    }
}

fn flatpak_user_remote_exists() -> Result<bool, String> {
    let output = Command::new("flatpak")
        .args(["remotes", "--user", "--columns=name"])
        .output()
        .map_err(|error| format!("could not inspect user Flatpak remotes: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|remote| remote.trim() == FLATHUB_REMOTE))
}

fn flathub_enabled(paths: &StorePaths) -> Result<bool, String> {
    if !paths.sources.is_file() {
        return Ok(false);
    }
    let output = Command::new("jq")
        .args(["-er", ".schemaVersion == 1 and .flathubEnabled == true"])
        .arg(&paths.sources)
        .output()
        .map_err(|error| format!("could not read Store sources: {error}"))?;
    Ok(output.status.success())
}

fn print_sources() {
    let result = (|| -> Result<(), String> {
        initialize_store_state()?;
        let paths = store_paths()?;
        let enabled = flathub_enabled(&paths)?;
        let remote = flatpak_user_remote_exists()?;
        println!(
            "{{\"schemaVersion\":{STORE_SOURCES_SCHEMA_VERSION},\"nixpkgs\":{{\"enabled\":true,\"default\":true}},\"flathub\":{{\"enabled\":{enabled},\"default\":false,\"userRemote\":{remote}}}}}"
        );
        Ok(())
    })();
    if let Err(error) = result {
        println!(
            "{{\"schemaVersion\":{STORE_SOURCES_SCHEMA_VERSION},\"nixpkgs\":{{\"enabled\":true,\"default\":true}},\"flathub\":{{\"enabled\":false,\"default\":false,\"userRemote\":false}},\"error\":\"{}\"}}",
            json_escape(&error)
        );
    }
}

fn enable_flathub() -> Result<(), String> {
    initialize_store_state()?;
    let paths = store_paths()?;
    let _lock = StoreLock::acquire(&paths)?;
    let status = Command::new("flatpak")
        .args([
            "remote-add",
            "--user",
            "--if-not-exists",
            FLATHUB_REMOTE,
            FLATHUB_REPO,
        ])
        .status()
        .map_err(|error| format!("could not enable Flathub: {error}"))?;
    if !status.success() {
        return Err("Flatpak rejected the Flathub remote".to_string());
    }
    if !flatpak_user_remote_exists()? {
        return Err("Flathub was not added to the user Flatpak installation".to_string());
    }
    atomic_private_write(
        &paths.sources,
        "{\"schemaVersion\":1,\"flathubEnabled\":true}\n",
    )
}

fn flatpak_user_installed(app_id: &str) -> bool {
    Command::new("flatpak")
        .args(["info", "--user", app_id])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn manifest_with_flatpak(path: &Path, app_id: &str) -> Result<String, String> {
    inspect_manifest(path)?;
    let output = Command::new("jq")
        .args([
            "--arg",
            "appId",
            app_id,
            r#".apps = ([.apps[] | select(.id != $appId)] + [{id:$appId,source:"flathub",flatpakId:$appId}])"#,
        ])
        .arg(path)
        .output()
        .map_err(|error| format!("could not update Store manifest: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    String::from_utf8(output.stdout).map_err(|_| "jq returned a non-UTF-8 manifest".to_string())
}

fn install_flatpak(app_id: &str) -> Result<(), String> {
    if !safe_identifier(app_id) || !app_id.contains('.') {
        return Err("invalid Flatpak application id".to_string());
    }
    initialize_store_state()?;
    let paths = store_paths()?;
    let _lock = StoreLock::acquire(&paths)?;
    if !flathub_enabled(&paths)? || !flatpak_user_remote_exists()? {
        return Err("Flathub is not enabled for this user".to_string());
    }

    let already_installed = flatpak_user_installed(app_id);
    let status = Command::new("flatpak")
        .args([
            "install",
            "--user",
            "--noninteractive",
            "--app",
            FLATHUB_REMOTE,
            app_id,
        ])
        .status()
        .map_err(|error| format!("could not install Flatpak application: {error}"))?;
    if !status.success() {
        return Err("Flatpak installation failed".to_string());
    }

    let manifest = match manifest_with_flatpak(&paths.manifest, app_id) {
        Ok(manifest) => manifest,
        Err(error) => {
            if !already_installed {
                let _ = Command::new("flatpak")
                    .args(["uninstall", "--user", "--noninteractive", app_id])
                    .status();
            }
            return Err(error);
        }
    };
    if let Err(error) = atomic_private_write(&paths.manifest, &manifest) {
        if !already_installed {
            let _ = Command::new("flatpak")
                .args(["uninstall", "--user", "--noninteractive", app_id])
                .status();
        }
        return Err(error);
    }
    Ok(())
}

fn print_install_flatpak(app_id: Option<String>) {
    let Some(app_id) = app_id else {
        usage();
    };
    match install_flatpak(&app_id) {
        Ok(()) => println!(
            "{{\"schemaVersion\":1,\"available\":true,\"source\":\"flathub\",\"id\":\"{}\"}}",
            json_escape(&app_id)
        ),
        Err(error) => println!(
            "{{\"schemaVersion\":1,\"available\":false,\"source\":\"flathub\",\"id\":\"{}\",\"error\":\"{}\"}}",
            json_escape(&app_id),
            json_escape(&error)
        ),
    }
}

fn print_enable_flathub() {
    match enable_flathub() {
        Ok(()) => print_sources(),
        Err(error) => println!(
            "{{\"schemaVersion\":{STORE_SOURCES_SCHEMA_VERSION},\"available\":false,\"error\":\"{}\"}}",
            json_escape(&error)
        ),
    }
}

fn print_search(query: &str) {
    let escaped_query = json_escape(query);
    match catalog_path() {
        Some(path) => match search_catalog(&path, query) {
            Ok(results) => println!(
                "{{\"available\":true,\"query\":\"{escaped_query}\",\"results\":{results}}}"
            ),
            Err(error) => println!(
                "{{\"available\":false,\"query\":\"{escaped_query}\",\"results\":[],\"error\":\"{}\"}}",
                json_escape(&error)
            ),
        },
        None => println!(
            "{{\"available\":false,\"query\":\"{escaped_query}\",\"results\":[],\"error\":\"catalogue path not configured\"}}"
        ),
    }
}

fn usage() -> ! {
    eprintln!(
        "usage: vesper-store-core <catalog-status|sources|flathub-enable|nix-install APP_ID|flatpak-install APP_ID|search QUERY|store-state|store-init|manifest-status>"
    );
    std::process::exit(2);
}

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("catalog-status") => print_catalog_status(),
        Some("sources") => print_sources(),
        Some("flathub-enable") => print_enable_flathub(),
        Some("nix-install") => print_install_nix(args.next()),
        Some("flatpak-install") => print_install_flatpak(args.next()),
        Some("search") => print_search(&args.collect::<Vec<_>>().join(" ")),
        Some("store-state") => print_store_state(),
        Some("store-init") => print_store_init(),
        Some("manifest-status") => print_manifest_status(),
        _ => usage(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_reject_command_text() {
        assert!(safe_identifier("org.example.App"));
        assert!(safe_identifier("hello-world_2"));
        assert!(!safe_identifier("org.example.App;touch"));
        assert!(!safe_identifier("../org.example.App"));
        assert!(!safe_identifier(""));
    }

    #[test]
    fn private_atomic_write_replaces_complete_state() {
        let directory = env::temp_dir().join(format!(
            "vesper-store-atomic-write-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("state.json");
        atomic_private_write(&path, "{\"first\":true}\n").unwrap();
        atomic_private_write(&path, "{\"second\":true}\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"second\":true}\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_file(path).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
