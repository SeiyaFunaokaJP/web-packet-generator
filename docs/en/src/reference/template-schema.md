# Template schema

Status: draft

Templates are YAML (or equivalent JSON) files with the extension `.wpg.yaml` and a top-level `schema: 1` version. A template holds metadata, a layer array, a transmit profile and variable definitions. A JSON Schema will be published and checked with `wpg template lint`. See the detailed specification under docs/spec for a full example.
