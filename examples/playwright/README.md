# Playwright example

Minimal Playwright Test project that annotates cases with Vulyk keys and emits
JUnit XML (`results.xml`) for [`vulyk-upload.sh`](../../scripts/vulyk-upload.sh).

For dogfooding against the Vulyk QA project with the built-in reporter, see
[`docs/dev/testing/playwright-vulyk.md`](../../docs/dev/testing/playwright-vulyk.md)
and `e2e/reporters/vulyk-reporter.ts`.

## Local

```bash
cd examples/playwright
npm install
npx playwright install chromium
npm test
# results.xml next to playwright.config.ts
```

## Report into Vulyk

```bash
export VULYK_API_URL=https://vulyk.example.com
export VULYK_API_TOKEN=vulyk_ci_…
export VULYK_PROJECT_KEY=KOC

VULYK_RUN_ID="$(../../scripts/vulyk-upload.sh --open-run --print-run-id \
  --suite-name "playwright-local" --provider other)"
export VULYK_RUN_ID

../../scripts/vulyk-upload.sh --file results.xml --format junit
../../scripts/vulyk-upload.sh --close-run
```

## CI

Copy a provider file from [`ci/`](ci/). Guide:
[framework-examples.md](../../docs/guides/framework-examples.md#playwright).
