# CML2 での注意点

Status: draft

最初のデプロイ先は Cisco CML2 上の Ubuntu ノードで、virtio 上の afpacket バックエンドが主経路になります。プロミスキャスモードの扱い、仮想リンクでの MAC アドレス制限、virtio のオフロード挙動に注意してください。`scripts/deploy.sh` はバイナリの転送、setcap、systemd unit の投入を行います。実測結果と注意点は M2 完了後にここへ記録します。
