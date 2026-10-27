# 法的注意

## 責任ある利用

wpg は、ネットワークの検証・試験を目的としたツールです。使用してよいのは、**自分が所有するネットワーク、または明示的に試験の許可を得たネットワークだけ**です。

本ツールはステートレスに任意のフレームを高速に送出でき、ARP / ND スイープ、SYN / MAC フラッド、不正なフレームなど、運用中のネットワークを乱し得るトラフィックを生成できます。許可なくこれらを他者のネットワークや機器に対して行うことは、多くの国・地域で違法です。利用に伴う責任はすべて利用者が負います。

- 新しいテンプレートは、送信前に必ず予行演習用のバックエンド (`--backend null`、または `-i out.pcapng` を指定した `pcapfile`) で確認してください。
- 本番ネットワークから隔離したラボで試験してください (CML2 などの仮想環境が有用です。[CML2 での注意点](operations/cml2.md))。
- 管理者設定の `[tx] max_rate` と `allowed_ifaces` で、使えるレートとインタフェースを制限できます ([設定リファレンス](reference/config.md))。

攻撃ツール化を目的とする変更は受け付けません (CONTRIBUTING.ja.md「責任ある利用」)。

## ライセンス

本ソフトウェアは [MIT](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/LICENSE-MIT) または [Apache-2.0](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/LICENSE-APACHE) のデュアルライセンスで、どちらかを選んで利用できます。コントリビューションは、別段の表明がない限り同じデュアルライセンスで提供されたものとみなします (Developer Certificate of Origin。`git commit -s` で sign-off してください)。

## 第三者コンポーネント

依存クレートはそれぞれのライセンスに従います。一覧とライセンスポリシーは `cargo deny check` (`deny.toml`) で検査しています。

Web ターミナルの画面には [xterm.js](https://xtermjs.org/) と、その fit / web-links / search アドオン (いずれも MIT ライセンス) を同梱しています。著作権表示とライセンス本文は `crates/wpg-web/vendor/xterm/LICENSE` にあり、wpg の配布物では `/xterm/LICENSE` として配信されます。

Windows の送信に使う **Npcap** は本ソフトウェアには同梱されず、利用者が [npcap.com](https://npcap.com/) から入手し、同社の規約のもとでインストールする必要があります。再配布には別途ライセンスが必要な場合があります。

## 免責

本ソフトウェアは、MIT / Apache-2.0 ライセンスに定めるとおり「現状のまま」提供され、明示黙示を問わずいかなる保証もありません。
