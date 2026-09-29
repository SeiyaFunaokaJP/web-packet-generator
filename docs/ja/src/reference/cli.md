# CLI リファレンス

Status: draft

`wpg` バイナリは serve、send、render、import、template、ifaces、doctor、bench の各サブコマンドを提供します。サーバとヘッドレス送出が同一バイナリで動くため、CI や ssh 先での実行に向いています。例えば `wpg send tpl.wpg.yaml -i eth0 --rate 1Mpps` でテンプレートを送出します。各コマンドのオプションは CLI 定義から生成する予定です。
