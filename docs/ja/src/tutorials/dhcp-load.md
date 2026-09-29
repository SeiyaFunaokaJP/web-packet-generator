# チュートリアル: DHCP 負荷試験

Status: draft

OUI を固定した random_mac ジェネレータで、大量の DHCP Discover を送出します。chaddr のクライアント MAC と Ethernet 送信元を、共有変数で一致させる方法を説明します。DHCP サーバやリレーの負荷時の挙動を測定することが目的です。応答の扱いは簡易的な受信カウンタまでとします。
