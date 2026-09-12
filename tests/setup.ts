import { afterEach } from "vitest";
import { cleanup } from "@testing-library/svelte";

// @testing-library/svelte only auto-registers cleanup when `afterEach` is a
// global (Jest). Vitest runs without globals here, so wire it up manually to
// keep renders from leaking between tests.
afterEach(() => {
  cleanup();
});
