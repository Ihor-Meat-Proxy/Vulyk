# vulyk CLI

Reporter for uploading JUnit XML or Vulyk JSON results to a Vulyk API.

```bash
cargo run -p vulyk-cli -- report --file report.xml --create-run --project-key DEMO
```

See [docs/guides/ingest-and-ci.md](../docs/guides/ingest-and-ci.md) for formats, env vars, the GitHub Action, and GitLab template.
