# Selenide / Maven example

Java 17 + Maven + Selenide + JUnit 5 suite that embeds Vulyk case keys and
uploads Surefire XML with [`scripts/vulyk-upload.sh`](../../scripts/vulyk-upload.sh).

## Local

```bash
cd examples/selenide-maven
mvn -B test
# Surefire XML: target/surefire-reports/TEST-*.xml
```

Requires Chrome/Chromium (Selenide downloads the matching driver).

## Report into Vulyk

```bash
export VULYK_API_URL=https://vulyk.example.com
export VULYK_API_TOKEN=vulyk_ci_…
export VULYK_PROJECT_KEY=KOC

VULYK_RUN_ID="$(../../scripts/vulyk-upload.sh --open-run --print-run-id \
  --suite-name "selenide-local" --provider other)"
export VULYK_RUN_ID

../../scripts/vulyk-upload.sh --file target/surefire-reports --format junit
../../scripts/vulyk-upload.sh --close-run
```

`--file` accepts a directory of Surefire `TEST-*.xml` files or a single XML path.

## CI

Copy a provider file from [`ci/`](ci/) into your automation repo and set secrets.
Full walkthrough: [framework-examples.md](../../docs/guides/framework-examples.md#selenide--junit-5-maven).
