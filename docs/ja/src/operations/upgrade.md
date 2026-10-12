# アップグレード

Status: draft

アップグレードでは `wpg` バイナリを置き換えてサーバを再起動します。設定、データベース、テンプレートはそのまま残ります。それぞれがバージョンを持っていて、自動で、またはコマンド 1 つで新しい形式に移行されます。

## アップグレードの前に

1. GitHub Release のページでリリースノートを読みます。
2. 設定とデータディレクトリをバックアップします。データベースは WAL モードの SQLite なので、先にサーバを止めてください (または `sqlite3 wpg.sqlite ".backup wpg.backup.sqlite"` でコピーします)。

   ```sh
   sudo systemctl stop wpg
   sudo tar -C / -czf wpg-backup-$(date +%F).tar.gz etc/wpg var/lib/wpg
   ```

3. 起動せずに、新しいバイナリで設定とテンプレートを確認します。

   ```sh
   ./wpg-new --version
   WPG_CONFIG=/etc/wpg/wpg.toml ./wpg-new config validate
   ./wpg-new template migrate --check /var/lib/wpg/templates /path/to/your/library
   ```

## インストール方法ごとの手順

| インストール方法 | アップグレード | ロールバック |
|---|---|---|
| `scripts/deploy.sh` | 新しいビルドで同じコマンドを再実行 | `scripts/deploy.sh <host> --rollback` |
| systemd (手動) | `sudo install -m 0755 wpg /usr/local/bin/wpg && sudo systemctl restart wpg` | 古いバイナリを入れ直す |
| launchd | `sudo install -m 0755 wpg /usr/local/bin/wpg && sudo launchctl kickstart -k system/io.github.seiyafunaokajp.wpg` | 古いバイナリで同じ操作 |
| Windows のタスク | `wpg-service.ps1 -Action Stop`、`wpg.exe` を置き換え、`-Action Start` | 古い `wpg.exe` で同じ操作 |
| Docker Compose | `git pull && docker compose up -d --build` (`wpg-data` ボリュームは残る) | 古いバージョンをチェックアウトして再ビルド |
| リリースアーカイブ | 新しいアーカイブを古いディレクトリに上書き展開 (macOS では quarantine 属性を再度外す) | 古いアーカイブを展開 |

Linux では、バイナリのファイルを置き換えるたびに `setcap` をやり直す必要があります (deploy.sh は自動で行います。systemd unit では不要です)。

## 移行されるもの

### データベース (`<data_dir>/wpg.sqlite`)

ユーザ、セッション、ジョブ履歴です。スキーマのバージョンは `schema_version` テーブルに保存されています。起動時に、未適用のマイグレーションをそれぞれ別のトランザクションで順に適用します。手動の作業はありません。**新しい** wpg が書いたデータベースは "database schema version N is newer than this build supports" で拒否されるので、ダウングレードにはアップグレード前のバックアップが必要です。

### 設定 (`wpg.toml`)

未知のキーはエラーになるので、タイプミスが黙って無視されることはありません。新しいバージョンは既定値付きのキーを追加するだけなので、古い `wpg.toml` はそのまま使えます。ダウングレードした場合は、古いバージョンが知らないキーがあると起動を拒否します。`wpg config validate` がそのキーを示します。新しいキーとその説明は、`wpg config init --path /tmp/wpg.toml` で作ったファイルと比べると確認できます。

### テンプレート (`schema:`)

テンプレートは必ず `schema: <n>` で始まります。テンプレートを読み込むとき (ライブラリ、ユーザテンプレート、バンドル、API)、wpg は古いバージョンを**メモリ上で**移行します。ファイルは書き換えません。GUI から保存したテンプレートは現在のバージョンで書かれます。バイナリが対応するより**新しい** `schema:` のテンプレートと、`schema:` の無いテンプレートは拒否されます ("unsupported template schema")。

ディスク上のファイルを書き換えるには (Git で管理しているライブラリなど) `wpg template migrate` を使います。

```sh
wpg template migrate --check templates/        # 古いファイルを一覧。1 つでもあれば終了ステータス 1
wpg template migrate templates/                # 書き換え。元のファイルは <file>.bak に残す
wpg template migrate --no-backup a.wpg.yaml    # .bak を残さない
```

ディレクトリは `*.wpg.yaml` / `*.wpg.json` を再帰的に探します。書き換えたファイルは YAML のコメントが失われ、キーが正規の順序になるので、差分を確認してください。現在のバージョンのファイルには触れません。

現在のテンプレートスキーマは **1** で、これまでに存在するのはこのバージョンだけなので、移行するものはまだありません。この仕組みは、将来フォーマットを変えたときに保存済みのテンプレートが壊れないようにするためのものです。

## 開発者向け: テンプレートのフォーマットを変えるとき

古いテンプレートをそのまま読めなくなる変更 (キーの改名・移動、意味の変更) には、新しいスキーマバージョンが必要です。

1. `crates/wpg-template/src/lib.rs` の `SCHEMA_VERSION` を上げます。
2. `MIGRATIONS` にステップを追加します。バージョン `n` の文書をバージョン `n + 1` に変換する (`schema: n + 1` の設定も含む) `fn(serde_yaml::Value) -> Result<serde_yaml::Value, TemplateError>` です。`MIGRATIONS[n - 1]` が `n` を移行します。ステップは順に実行されるので、各ステップは隣のバージョンだけを知っていれば十分です。
3. バージョン `n` のフィクスチャを読み込み、期待するバージョン `n + 1` のテンプレートと比べるテストを追加し、フィクスチャは残しておきます。
4. サンプルテンプレートを更新し (`wpg template migrate templates/`)、`docs/schema/template.schema.json` を再生成し (`wpg template schema`)、テンプレートスキーマのリファレンスを日英とも更新します。
5. リリースノートに変更を書きます。

追加だけの変更 (既定値のある任意のキーを足すなど) には新しいバージョンは不要です。

データベースの変更も同じ考え方です。`crates/wpg-server/src/db.rs` の `MIGRATIONS` に SQL のマイグレーションを追加します。リリース済みのマイグレーションは編集しないでください。
