# Upgrading

Status: draft

An upgrade replaces the `wpg` binary and restarts the server. Configuration, the database and templates stay where they are; each of them carries a version and is migrated forward automatically or with one command.

## Before you upgrade

1. Read the release notes on the GitHub Release page.
2. Back up the configuration and the data directory. The database is SQLite in WAL mode, so stop the server first (or copy with `sqlite3 wpg.sqlite ".backup wpg.backup.sqlite"`):

   ```sh
   sudo systemctl stop wpg
   sudo tar -C / -czf wpg-backup-$(date +%F).tar.gz etc/wpg var/lib/wpg
   ```

3. Check the new binary against your configuration and templates without starting it:

   ```sh
   ./wpg-new --version
   WPG_CONFIG=/etc/wpg/wpg.toml ./wpg-new config validate
   ./wpg-new template migrate --check /var/lib/wpg/templates /path/to/your/library
   ```

## Steps per installation method

| Installed with | Upgrade | Roll back |
|---|---|---|
| `scripts/deploy.sh` | Re-run the same command with the new build | `scripts/deploy.sh <host> --rollback` |
| systemd by hand | `sudo install -m 0755 wpg /usr/local/bin/wpg && sudo systemctl restart wpg` | Re-install the old binary |
| launchd | `sudo install -m 0755 wpg /usr/local/bin/wpg && sudo launchctl kickstart -k system/io.github.seiyafunaokajp.wpg` | Same with the old binary |
| Windows scheduled task | `wpg-service.ps1 -Action Stop`, replace `wpg.exe`, `-Action Start` | Same with the old `wpg.exe` |
| Docker Compose | `git pull && docker compose up -d --build` (the `wpg-data` volume is kept) | Check out the old version and rebuild |
| Release archive | Extract the new archive over the old directory (macOS: clear the quarantine flag again) | Extract the old archive |

On Linux, `setcap` must be re-applied whenever the binary file is replaced (deploy.sh does it; the systemd unit does not need it).

## What is migrated

### Database (`<data_dir>/wpg.sqlite`)

Users, sessions and job history. The schema version is stored in the `schema_version` table. On start, the server applies the missing migrations in order, each in its own transaction. Nothing to do by hand. A database written by a **newer** wpg is refused with "database schema version N is newer than this build supports", so a downgrade needs the backup taken before the upgrade.

### Configuration (`wpg.toml`)

Unknown keys are errors, so a typo is never silently ignored. New versions only add keys with defaults, so an old `wpg.toml` keeps working. After a downgrade, keys that the old version does not know make it refuse to start; `wpg config validate` names them. Compare your file with `wpg config init --path /tmp/wpg.toml` to see new keys and their documentation.

### Templates (`schema:`)

Every template starts with `schema: <n>`. When a template is loaded (library, user templates, bundles, the API), wpg upgrades older versions **in memory**; nothing is rewritten. Templates saved from the GUI are written with the current version. A template with a **newer** `schema:` than the binary supports is rejected ("unsupported template schema"), as is one without `schema:`.

To rewrite files on disk (for example a library kept in Git), use `wpg template migrate`:

```sh
wpg template migrate --check templates/        # list outdated files; exit status 1 if any
wpg template migrate templates/                # rewrite them, keeping <file>.bak
wpg template migrate --no-backup a.wpg.yaml    # no .bak copy
```

Directories are scanned recursively for `*.wpg.yaml` / `*.wpg.json`. Rewritten files lose their YAML comments and get the canonical key order, so review the diff. Files already at the current version are not touched.

The current template schema is **1**, the only version so far, so there is nothing to migrate yet. The mechanism exists so that a future format change does not break saved templates.

## For developers: changing the template format

A change that old templates cannot be read with (renamed or moved keys, changed meaning) needs a new schema version:

1. Increase `SCHEMA_VERSION` in `crates/wpg-template/src/lib.rs`.
2. Append a step to `MIGRATIONS`: a `fn(serde_yaml::Value) -> Result<serde_yaml::Value, TemplateError>` that turns a version `n` document into version `n + 1`, including setting `schema: n + 1`. `MIGRATIONS[n - 1]` upgrades `n`; steps run in sequence, so each one only knows its neighbour.
3. Add a test that loads a version `n` fixture and compares it with the expected version `n + 1` template, and keep the fixture.
4. Update the sample templates (`wpg template migrate templates/`), regenerate `docs/schema/template.schema.json` (`wpg template schema`), and update the template schema reference in both languages.
5. Mention the change in the release notes.

Additive changes (a new optional key with a default) do not need a new version.

Database changes work the same way: append an SQL migration to `MIGRATIONS` in `crates/wpg-server/src/db.rs`; never edit one that has been released.
