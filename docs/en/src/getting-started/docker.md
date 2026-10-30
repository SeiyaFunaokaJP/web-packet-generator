# Run with Docker

The image builds wpg and its web UI inside the container, so the host needs Docker and nothing else: no Rust toolchain, no trunk. Use a **Linux host**: Docker Desktop on macOS and Windows runs containers in a VM, and frames sent there never reach your physical NIC.

This page is the short path. Build options, proxies, every setting and the limits are in [Docker](../operations/docker.md).

## 1. Get the repository and start

```sh
git clone https://github.com/SeiyaFunaokaJP/web-packet-generator.git
cd web-packet-generator
WPG_PASSWORD='change me' WPG_ALLOWED_IFACES='["eth1"]' docker compose up -d --build
```

- `WPG_PASSWORD` is required: the server listens on every address of the host, so it refuses to start without a password. Pick your own; it is the administrator password of the web UI.
- `WPG_ALLOWED_IFACES` lists the interfaces jobs may send on. Use the name of your lab interface (`ip -br link` lists them); leave it out to allow all.
- The first build compiles everything and takes a while. Later starts reuse the image.

The container uses the host network and gets only `CAP_NET_RAW` and `CAP_NET_ADMIN`, so it can send on the host's interfaces without running wpg as root on the host.

## 2. Check it

```sh
docker compose ps                         # STATUS shows "healthy" once the server answers
docker compose exec wpg wpg doctor        # raw sockets and interfaces, as the container sees them
```

Open <http://127.0.0.1:8080> on the host (or `http://<host>:8080` from another machine) and log in with the password. [First login](first-login.md) walks through the screens.

## 3. Try a template

The bundled templates are in `/usr/share/wpg/templates` inside the container and appear in the **Templates** screen. From the command line, render one to a file first (nothing is sent), then send it:

```sh
docker compose exec wpg wpg render /usr/share/wpg/templates/examples/arp-sweep.wpg.yaml -o /var/lib/wpg/arp.pcapng
docker compose exec wpg wpg send /usr/share/wpg/templates/examples/arp-sweep.wpg.yaml -i eth1 --rate 1000pps
```

`/var/lib/wpg` is the `wpg-data` volume: the database, your own templates and files such as the pcapng above survive restarts and image upgrades. Only send on networks you own or are authorised to test. Continue with the [ARP sweep tutorial](../tutorials/arp-sweep.md), using `docker compose exec wpg wpg ...` where the tutorial runs `wpg ...`.

## Stop, upgrade, remove

```sh
docker compose logs -f wpg                # follow the log
docker compose down                       # stop; the wpg-data volume is kept
git pull && WPG_PASSWORD='change me' docker compose up -d --build   # upgrade
docker compose down -v                    # stop and delete the wpg-data volume
```

To keep the settings out of your shell history, put them in a `.env` file next to `docker-compose.yml` (`WPG_PASSWORD=...`, `WPG_ALLOWED_IFACES=...`, `WPG_BIND=...`). The repository ignores `.env` in both git and the Docker build context, so the password is neither committed nor copied into the image.
