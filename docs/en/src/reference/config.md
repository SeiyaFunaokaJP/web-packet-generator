# Configuration

Status: draft

Configuration lives in `wpg.toml` and can be overridden by environment variables and CLI options. `wpg config init` writes a fully commented example file. Settings cover the bind address, TLS, authentication, backend options such as DPDK EAL arguments, and template library paths. Multiple library paths allow a Git-managed directory to be loaded directly.
