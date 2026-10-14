<script lang="ts">
  import { toastStore, type Toast } from "$lib/stores/toast";
  import {
    AlertCircle,
    AlertTriangle,
    CheckCircle,
    Info,
    X,
  } from "@lucide/svelte";

  let toasts: Toast[] = [];

  toastStore.subscribe((state) => {
    toasts = state.toasts;
  });

  function getIcon(type: string) {
    switch (type) {
      case "success":
        return CheckCircle;
      case "error":
        return AlertCircle;
      case "warning":
        return AlertTriangle;
      case "info":
      default:
        return Info;
    }
  }

  function removeToast(id: string) {
    toastStore.remove(id);
  }
</script>

<div class="toast-container">
  {#each toasts as toast (toast.id)}
    <div
      class="toast"
      class:success={toast.type === "success"}
      class:error={toast.type === "error"}
      class:info={toast.type === "info"}
      class:warning={toast.type === "warning"}
    >
      <div class="toast-icon">
        <svelte:component this={getIcon(toast.type)} size={20} />
      </div>
      <div class="toast-message">
        <!--
          Most toast text carries names and error strings from Nexus, mod.io or
          a shared modpack, so it renders as text. Only a message explicitly
          flagged as trusted markup reaches the HTML sink.
        -->
        {#if toast.html}
          {@html toast.message}
        {:else}
          {toast.message}
        {/if}
      </div>
      {#if toast.action}
        <button
          class="toast-action"
          on:click={() => {
            try {
              void toast.action?.handler();
            } finally {
              removeToast(toast.id);
            }
          }}
        >
          {toast.action.label}
        </button>
      {/if}
      <button
        class="toast-close"
        on:click={() => removeToast(toast.id)}
        aria-label="Close notification"
      >
        <X size={16} />
      </button>
    </div>
  {/each}
</div>
