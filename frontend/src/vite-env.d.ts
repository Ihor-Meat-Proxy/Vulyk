/// <reference types="vite/client" />
/// <reference types="vite-plugin-pwa/client" />

interface ImportMetaEnv {
  readonly VITE_API_URL?: string
  /** When `"true"`, omit `data-testid` / list data-* attrs from the DOM. Default: keep them. */
  readonly VITE_STRIP_TEST_IDS?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
