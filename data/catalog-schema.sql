PRAGMA foreign_keys = ON;
PRAGMA user_version = 1;

CREATE TABLE apps (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    generic_name TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    appstream_id TEXT,
    desktop_id TEXT,
    homepage TEXT,
    icon_key TEXT,
    primary_category TEXT
) STRICT;

CREATE TABLE variants (
    id INTEGER PRIMARY KEY,
    app_id TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL,
    source_id TEXT NOT NULL,
    package_attr TEXT,
    package_version TEXT,
    flatpak_id TEXT,
    license TEXT,
    sandbox_kind TEXT NOT NULL,
    install_kind TEXT NOT NULL,
    supported INTEGER NOT NULL DEFAULT 1 CHECK (supported IN (0, 1)),
    broken INTEGER NOT NULL DEFAULT 0 CHECK (broken IN (0, 1)),
    insecure INTEGER NOT NULL DEFAULT 0 CHECK (insecure IN (0, 1)),
    UNIQUE (source_kind, source_id)
) STRICT;

CREATE TABLE categories (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL
) STRICT;

CREATE TABLE app_categories (
    app_id TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
    PRIMARY KEY (app_id, category_id)
) STRICT;

CREATE TABLE screenshots (
    id INTEGER PRIMARY KEY,
    app_id TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    caption TEXT NOT NULL DEFAULT '',
    position INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE keywords (
    app_id TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    keyword TEXT NOT NULL,
    PRIMARY KEY (app_id, keyword)
) STRICT;

CREATE TABLE aliases (
    app_id TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    alias TEXT NOT NULL,
    PRIMARY KEY (app_id, alias)
) STRICT;

CREATE VIRTUAL TABLE apps_fts USING fts5(
    app_id UNINDEXED,
    name,
    generic_name,
    aliases,
    keywords,
    package_attr,
    summary,
    description,
    tokenize = 'unicode61 remove_diacritics 2'
);
