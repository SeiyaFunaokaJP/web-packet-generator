# Install on Linux

Status: draft

The minimum supported distribution is Ubuntu 20.04 or later (glibc 2.31). Install the release binary and grant raw-socket capabilities with `setcap cap_net_raw,cap_net_admin+ep` so wpg can run without root. The libpcap development package is needed only when building from source. Run `wpg doctor` afterwards to confirm permissions.
