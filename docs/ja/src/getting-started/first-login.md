# 初回ログイン

Status: draft

管理パスワードは `--password`、環境変数、設定ファイル、初回アクセス時のセットアップ画面のいずれかで設定します。未設定の場合、wpg は localhost のみに bind し警告を表示します。セッションは HttpOnly / SameSite=Strict の Cookie で管理され、パスワードは argon2id でハッシュ化されます。ログイン後はダッシュボードでインタフェース状態を確認してください。
