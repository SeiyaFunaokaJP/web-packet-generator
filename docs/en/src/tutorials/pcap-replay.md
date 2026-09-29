# Tutorial: pcap to template replay

Status: draft

This tutorial imports a pcap or pcapng capture with `wpg import` and turns each packet into a template with decoded layers and fields. Generator inference from differences between packets is best-effort, so review the result before use. You then replay the template with `wpg send` or preview it with `wpg render`. Pasting a Wireshark hex stream is shown as an alternative import.
