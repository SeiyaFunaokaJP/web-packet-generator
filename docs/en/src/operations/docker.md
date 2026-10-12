# Docker

Status: draft

The repository has a multi-stage `Dockerfile` and a `docker-compose.yml`. The build stage (`rust:1-bookworm`) downloads Trunk, builds the web UI and the release binary; the runtime image (`debian:bookworm-slim`) holds only `wpg`, `curl` (for the health check) and the sample templates in `/usr/share/wpg/templates`.

## Build

```sh
docker build -t wpg .
docker build -t wpg --build-arg FEATURES=pcap .     # with the libpcap backend
```

The build needs Internet access: crates.io, the Trunk release on GitHub, and the Tailwind CSS and wasm-bindgen tools that Trunk downloads. Behind a TLS-intercepting proxy, pass the proxy's CA certificate (PEM) as a build secret and the proxy address as the standard build arguments. The certificate is trusted in the build stage only and does not end up in the runtime image:

```sh
docker build -t wpg --secret id=ca_certs,src=/path/to/proxy-ca.pem \
  --build-arg HTTPS_PROXY=http://proxy.example:3128 .
```

If Docker Hub rate-limits anonymous pulls, the base images can be taken from another registry with `--build-arg RUST_IMAGE=mirror.gcr.io/library/rust:1-bookworm --build-arg RUNTIME_IMAGE=mirror.gcr.io/library/debian:bookworm-slim`.

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
- `wpg doctor` inside the container reports the container's view (capabilities, interfaces). The image is built without the `dpdk` feature (it would need the DPDK libraries in both stages); hugepages and `vfio-pci` for DPDK are prepared on the host in any case.
- Without `--network host` (the default bridge network with `-p 8080:8080`) the GUI works, but jobs can only send on the container's own `eth0`.

## Verification status

Checked in October 2026 (Docker 29.8, Linux 6.18 host, image without extra features):

- `docker build` completed (Trunk web UI and `cargo build --release --locked`); the runtime image is about 170 MB on disk (44 MB compressed).
- `docker run --network host --cap-drop ALL --cap-add NET_RAW --cap-add NET_ADMIN -e WPG_PASSWORD=...`: the health check turned `healthy`, `/api/health` and the web UI answered, and `wpg doctor` inside the container reported raw sockets with `CAP_NET_RAW` / `CAP_NET_ADMIN`.
- `wpg send` with the `afpacket` backend on `lo` inside the container sent 100 frames, and `wpg capture` in the same container received them.
- `--build-arg FEATURES=pcap`: the image ships `libpcap0.8`, `wpg doctor` finds it, and `wpg send --backend pcap` sent 50 frames on `lo`.
- `docker compose up -d --no-build`: the service turned `healthy`, login with `WPG_PASSWORD` succeeded (204) and a wrong password was refused (401). Without `WPG_PASSWORD`, compose refuses to start with the message from the compose file.

The development sandbox only allows HTTPS through its proxy, so `apt-get` there had to be pointed at `https://deb.debian.org` with a throw-away copy of the Dockerfile; the `Dockerfile` in the repository was otherwise used unchanged. Docker Desktop (macOS / Windows) was not checked.
