<script lang="ts">
  import { toastStore, type Toast } from "$lib/stores/toast";
  import { AlertCircle, CheckCircle, Info, X } from "lucide-svelte";

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
    >
      <div class="toast-icon">
        <svelte:component this={getIcon(toast.type)} size={20} />
      </div>
      <div class="toast-message">
        {toast.message}
      </div>
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
