# テンプレートスキーマ

Status: draft

テンプレートは拡張子 `.wpg.yaml` の YAML (または同等の JSON) ファイルで、最上位に `schema: 1` のバージョンを持ちます。メタ情報、レイヤ配列、送出プロファイル、変数定義で構成されます。JSON Schema が公開され、`wpg template lint` で検証できます。完全な例は docs/spec の詳細仕様を参照してください。
