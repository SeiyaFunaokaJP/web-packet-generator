# Tutorial: DHCP load

Status: draft

This tutorial sends a large number of DHCP Discover messages with a random_mac generator that keeps a fixed OUI. It explains how the client MAC in the chaddr field and the Ethernet source stay consistent through a shared variable. The goal is to measure how a DHCP server or relay behaves under load. Reply handling is limited to the simple receive counters.
