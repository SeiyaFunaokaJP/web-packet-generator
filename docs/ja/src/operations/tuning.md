# 性能チューニング

Status: draft

スループットは、コアの pin、バッチサイズ、リング深さ、TX キュー (RSS) 数に左右されます。レート制御はトークンバケットと busy-poll / nanosleep のハイブリッドで、その精度は UI に表示されます。`wpg bench` で各バックエンドを計測し、docs/perf.md のベースラインと比較してください。参考値は afpacket で 1〜3 Mpps/core 程度、DPDK で 10 Mpps/core 以上です。
