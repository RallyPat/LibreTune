import tsParser from '@typescript-eslint/parser';
import tsPlugin from '@typescript-eslint/eslint-plugin';
import reactHooksPlugin from 'eslint-plugin-react-hooks';

export default [
  {
    ignores: ['dist/**', 'node_modules/**', 'public/**', 'src-tauri/**'],
  },
  {
    files: ['src/**/*.{ts,tsx}'],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: 'latest',
        sourceType: 'module',
      },
    },
    plugins: {
      '@typescript-eslint': tsPlugin,
      'react-hooks': reactHooksPlugin,
    },
    rules: {
      ...tsPlugin.configs.recommended.rules,
      // tsconfig already enforces noUnusedLocals/noUnusedParameters; this
      // rule only adds the `_`-prefix escape hatch the codebase already uses.
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_', caughtErrors: 'none' },
      ],
      // ~145 existing `any`s, mostly in tests and Tauri mocks. Off until
      // someone wants to burn them down; flip to 'warn' to see the list.
      '@typescript-eslint/no-explicit-any': 'off',
      // Classic hooks rules only. The React Compiler rules that v7's
      // `recommended` preset adds (set-state-in-effect, refs, immutability,
      // purity, ...) flag ~140 existing sites and are not worth a big-bang
      // rewrite. Enable them once the compiler is actually adopted.
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',
    },
  },
];
