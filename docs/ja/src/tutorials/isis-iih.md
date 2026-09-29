# チュートリアル: IS-IS IIH の生成

Status: draft

802.2 LLC (DSAP/SSAP 0xFE) 上の IS-IS LAN Hello (L1/L2 IIH) を作成します。Area Address、Protocols Supported、IP Interface Address の各 TLV を設定し、PDU 長とパディングは wpg に自動計算させます。CML2 ラボ内のルータで隣接関係の挙動を確認する用途を想定しています。P2P IIH は応用例として扱います。
