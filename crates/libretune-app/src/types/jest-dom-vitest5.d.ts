// Vitest 5 compat shim for @testing-library/jest-dom type augmentation.
//
// Vitest 5 changed the `Assertion` interface to two type parameters
// (`<R extends void | Promise<void> = void, T = unknown>`), while
// @testing-library/jest-dom (7.0.1) still augments it with a single
// parameter (`<T = any>`). TypeScript requires identical type-parameter
// lists to merge interface declarations, so jest-dom's augmentation is
// silently dropped (masked by skipLibCheck) and every DOM matcher
// (toBeInTheDocument, toHaveValue, ...) loses its type.
//
// This re-declares the augmentation with vitest 5's exact signature.
// DELETE this file once jest-dom ships a vitest-5-compatible release.
import type { TestingLibraryMatchers } from '@testing-library/jest-dom/matchers';

declare module 'vitest' {
  // Empty interface with only a supertype is intentional: this is a
  // declaration-merge augmentation (same pattern jest-dom itself uses),
  // and the type parameters must mirror vitest 5's Assertion exactly.
  // eslint-disable-next-line @typescript-eslint/no-empty-object-type, @typescript-eslint/no-unused-vars
  interface Assertion<R extends void | Promise<void> = void, T = unknown>
    extends TestingLibraryMatchers<any, T> {}
}
