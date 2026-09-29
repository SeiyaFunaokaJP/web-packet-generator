# CLI reference

Status: draft

The `wpg` binary provides the subcommands serve, send, render, import, template, ifaces, doctor and bench. The same binary runs the server and headless sending, which suits CI and ssh sessions. For example, `wpg send tpl.wpg.yaml -i eth0 --rate 1Mpps` transmits a template. Per-command options will be generated from the CLI definition.
