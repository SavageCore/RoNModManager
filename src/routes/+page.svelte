<main
  class="mx-auto flex min-h-screen w-full max-w-5xl flex-col gap-6 px-6 py-10 sm:px-10"
>
  <header
    class="rounded-2xl border border-teal-700/15 bg-[var(--color-surface)] p-6 shadow-sm"
  >
    <p class="text-sm font-semibold uppercase tracking-[0.2em] text-teal-700">
      RoN Mod Manager
    </p>
    <h1 class="mt-2 text-3xl font-semibold">Phase 1 Scaffold Ready</h1>
    <p class="mt-3 max-w-3xl text-[var(--color-muted)]">
      Tauri v2 + Svelte 5 has been bootstrapped with a <code>pnpm</code>-first
      toolchain, linting, formatting, and test infrastructure.
    </p>
  </header>

  <section class="grid gap-4 sm:grid-cols-2">
    <article
      class="rounded-2xl border border-teal-700/15 bg-[var(--color-surface)] p-5"
    >
      <h2 class="text-lg font-semibold">Configured</h2>
      <ul class="mt-3 space-y-2 text-sm text-[var(--color-muted)]">
        <li>Tailwind CSS 4 via <code>@tailwindcss/vite</code></li>
        <li>ESLint + Prettier + Svelte check scripts</li>
        <li>Vitest unit-test harness</li>
        <li>Husky + lint-staged pre-commit hook</li>
      </ul>
    </article>
    <article
      class="rounded-2xl border border-teal-700/15 bg-[var(--color-surface)] p-5"
    >
      <h2 class="text-lg font-semibold">Next</h2>
      <ul class="mt-3 space-y-2 text-sm text-[var(--color-muted)]">
        <li>Implement Rust service layer (`models/`, `services/`)</li>
        <li>Add Tauri IPC commands in `src-tauri/src/commands/`</li>
        <li>Build Gale-inspired app layout and pages</li>
      </ul>
    </article>
  </section>
</main>
