# CML2 notes

Status: draft

The first deployment target is Ubuntu running as a node in Cisco CML2, where the afpacket backend over virtio is the main path. Keep in mind promiscuous-mode handling, MAC address restrictions on virtual links, and virtio offload behavior. `scripts/deploy.sh` copies the binary, applies setcap and installs a systemd unit. Measured results and pitfalls will be recorded here after M2.
