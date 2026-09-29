# Permissions

Status: draft

Sending raw frames is a privileged operation and the requirement differs per OS. Linux uses CAP_NET_RAW and CAP_NET_ADMIN, Windows needs Administrator plus Npcap, and macOS needs access to `/dev/bpf*`. At startup wpg runs a self-diagnosis and shows OS-specific guidance in the GUI. Administrators can also restrict allowed interfaces and the maximum rate.
