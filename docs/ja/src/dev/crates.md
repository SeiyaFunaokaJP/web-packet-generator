# crate 構成

Status: draft

リポジトリは `crates/` 配下の Cargo workspace です。wpg-core にはプロトコル IR、合成器、解析器、ジェネレータがあり、wasm32 でもビルドできる必要があります。その他に wpg-protocols、wpg-tx、wpg-rx、wpg-template、wpg-server、wpg-cli、wpg-web があります。locales、scripts、docs、templates、tests は `crates/` と同じ階層に置かれます。
