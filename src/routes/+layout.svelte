<script lang="ts">
  import "../app.css";
  import { resolve } from "$app/paths";
  import { page } from "$app/stores";

  const nav = [
    { href: "/", label: "Dashboard" },
    { href: "/mods", label: "Mods" },
    { href: "/collections", label: "Collections" },
    { href: "/settings", label: "Settings" },
  ];
</script>

<div class="mx-auto flex min-h-screen w-full max-w-7xl gap-4 px-4 py-6 sm:px-6">
  <aside
    class="sticky top-6 h-[calc(100vh-3rem)] w-56 rounded-2xl border border-teal-700/15 bg-[var(--color-surface)] p-4 shadow-sm"
  >
    <p class="text-xs font-semibold uppercase tracking-[0.18em] text-teal-700">
      RoN Mod Manager
    </p>
    <nav class="mt-4 flex flex-col gap-2 text-sm">
      {#each nav as item (item.href)}
        <a
          href={resolve(item.href)}
          class={`rounded-lg px-3 py-2 ${
            $page.url.pathname === item.href
              ? "bg-teal-700 text-white"
              : "text-[var(--color-muted)] hover:bg-zinc-100"
          }`}
        >
          {item.label}
        </a>
      {/each}
    </nav>
  </aside>

  <main class="min-w-0 flex-1">
    <slot />
  </main>
</div>
