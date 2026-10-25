# vulyk-cli

CLI and result reporter for Vulyk. Phase 1 supports `version` and a `report` stub; workstream **W4** adds JUnit / Vulyk JSON upload.

```bash
cargo run -p vulyk-cli -- version
cargo run -p vulyk-cli -- report --file results.xml --run-id <uuid>
```
