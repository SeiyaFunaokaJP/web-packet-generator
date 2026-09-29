# 権限設定

Status: draft

raw フレームの送出は特権操作であり、必要な権限は OS ごとに異なります。Linux は CAP_NET_RAW / CAP_NET_ADMIN、Windows は管理者権限と Npcap、macOS は `/dev/bpf*` へのアクセス権が必要です。wpg は起動時に自己診断を行い、OS 別の対処方法を GUI に表示します。管理者設定で、許可インタフェースや最大レートを制限することもできます。
