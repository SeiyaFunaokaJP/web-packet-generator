# Docker

Status: draft

The repository has a multi-stage `Dockerfile` and a `docker-compose.yml`. The build stage (`rust:1-bookworm`) downloads Trunk, builds the web UI and the release binary; the runtime image (`debian:bookworm-slim`) holds only `wpg`, `curl` (for the health check) and the sample templates in `/usr/share/wpg/templates`.

## Build

```sh
docker build -t wpg .
docker build -t wpg --build-arg FEATURES=pcap .     # with the libpcap backend
```

The build needs Internet access: crates.io, the Trunk release on GitHub, and the Tailwind CSS and wasm-bindgen tools that Trunk downloads. Behind a TLS-intercepting proxy, add the proxy's CA certificate to the build stage first.

## Run

Sending on the host's interfaces needs the host network namespace and two capabilities. Everything else is dropped:

```sh
docker run -d --name wpg --network host \
  --cap-drop ALL --cap-add NET_RAW --cap-add NET_ADMIN \
  -e WPG_PASSWORD='change me' \
  -e WPG_TX__ALLOWED_IFACES='["eth1"]' \
  -v wpg-data:/var/lib/wpg \
  wpg
```

Or with Compose (reads `WPG_PASSWORD`, `WPG_BIND`, `WPG_ALLOWED_IFACES` and `WPG_FEATURES` from the environment or a `.env` file):

```sh
WPG_PASSWORD='change me' WPG_ALLOWED_IFACES='["eth1"]' docker compose up -d --build
docker compose ps          # shows "healthy" once /api/health answers
docker compose logs -f wpg
```

| Setting | Image default | Notes |
|---|---|---|
| `WPG_SERVER__BIND` | `0.0.0.0:8080` | With `--network host` this is the host's port. Use `127.0.0.1:8080` to keep it local. |
| `WPG_PASSWORD` | none | Required for a non-loopback bind: the server refuses to start otherwise. A plain password or an argon2id hash. `WPG_AUTH__PASSWORD_FILE=/run/secrets/...` works with Docker secrets. |
| `WPG_DATA_DIR` | `/var/lib/wpg` (volume) | Database, user templates, TLS certificate. |
| `WPG_TEMPLATES__LIBRARY_PATHS` | `["/usr/share/wpg/templates"]` | Mount your own library and add its path. |
| `WPG_TX__ALLOWED_IFACES` | all | Interfaces jobs may send on. |

Other settings: any `WPG_<SECTION>__<KEY>` variable, or mount a `wpg.toml` and set `WPG_CONFIG`. See the [configuration reference](../reference/config.md).

The process runs as root inside the container, but with only `CAP_NET_RAW` and `CAP_NET_ADMIN`. A non-root user would need file capabilities on the binary, which fail to execute when the container's capability set lacks one of them, so the image keeps it simple.

## Limits

- Use Linux hosts. Docker Desktop on macOS and Windows runs containers inside a VM: `--network host` there is the VM's network, so frames never reach your physical NIC.
- `wpg doctor` inside the container reports the container's view (capabilities, interfaces). Hugepages and `vfio-pci` for the future DPDK backend have to be prepared on the host.
- Without `--network host` (the default bridge network with `-p 8080:8080`) the GUI works, but jobs can only send on the container's own `eth0`.

## Verification status

The compose file was checked with `docker compose config`. A complete `docker build` could not be finished in the development sandbox, whose TLS-intercepting proxy is not trusted inside the build container (the Trunk download fails). The build commands were checked on the host instead (`scripts/release.sh` runs the same `trunk build --release` and `cargo build --release --locked -p wpg-cli`), and the runtime stage was checked with a locally built binary. **Run `docker build` in a normal environment before relying on it.**
