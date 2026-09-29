# Tutorial: ARP sweep

Status: draft

This tutorial builds an ARP request template whose target IP uses a range generator such as 192.168.1.1-192.168.5.254. You will stack Ethernet and ARP layers in the builder and send the stream on a lab interface. Optionally wrap it in an 802.1Q tag to sweep across a VLAN. The result is checked with the receive counters.
