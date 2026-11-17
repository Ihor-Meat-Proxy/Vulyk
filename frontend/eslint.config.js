import js from '@eslint/js'
import react from 'eslint-plugin-react'
import reactHooks from 'eslint-plugin-react-hooks'
import jsxA11y from 'eslint-plugin-jsx-a11y'
import tseslint from 'typescript-eslint'

/** ESLint flat config — runs alongside oxlint (CI: npm run lint:eslint). */
export default tseslint.config(
  {
    ignores: [
      'dist/**',
      'coverage/**',
      'node_modules/**',
      'lighthouse-report.json',
      'scripts/**',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ['src/**/*.{ts,tsx}'],
    languageOptions: {
      parserOptions: {
        ecmaFeatures: { jsx: true },
      },
    },
    plugins: {
      react,
      'react-hooks': reactHooks,
      'jsx-a11y': jsxA11y,
    },
    settings: {
      // Pin React major for eslint-plugin-react (ESLint 10 removed
      // context.getFilename(); "detect" still calls it in 7.37.x).
      react: { version: '19.0' },
    },
    rules: {
      ...react.configs.recommended.rules,
      // Hooks 7 `recommended` enables React Compiler rules (set-state-in-effect,
      // etc.) that are still noisy on this codebase — keep the classic pair for
      // now and ratchet the rest later (#234).
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',
      ...jsxA11y.configs.recommended.rules,
      'react/react-in-jsx-scope': 'off',
      'react/prop-types': 'off',
      // Day-one: keep noise low; ratchet toward error later.
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],
      '@typescript-eslint/no-explicit-any': 'off',
      'jsx-a11y/click-events-have-key-events': 'warn',
      'jsx-a11y/no-static-element-interactions': 'warn',
      'jsx-a11y/label-has-associated-control': 'warn',
      // Prefer in-app Dialog / ConfirmDialog / PromptDialog (bee theme).
      'no-alert': 'error',
      'no-restricted-globals': [
        'error',
        {
          name: 'alert',
          message:
            'Use in-app Dialog components instead of window.alert / alert().',
        },
        {
          name: 'confirm',
          message:
            'Use useConfirmDialog() / ConfirmDialog instead of window.confirm / confirm().',
        },
        {
          name: 'prompt',
          message:
            'Use usePromptDialog() / PromptDialog instead of window.prompt / prompt().',
        },
      ],
    },
  },
)
