<script lang="ts">
  import SetupWizard from "$lib/components/SetupWizard.svelte";
  import {
    closeTour,
    currentStep,
    deferSetup,
    nextStep,
    prevStep,
    tourState,
  } from "$lib/tour/tourEngine";
  import { onDestroy, onMount, tick } from "svelte";

  const CARD_WIDTH = 380;
  // A setup card carries a form, so it gets the room a form needs.
  const SETUP_CARD_WIDTH = 560;
  const GAP = 14;
  const RING_PAD = 6;
  const VIEWPORT_MARGIN = 12;

  let cardEl: HTMLDivElement | null = null;
  let cardHeight = 150;

  $: rect = $tourState.rect;
  $: step = $currentStep;
  $: isLastStep = $tourState.index + 1 >= $tourState.total;
  $: cardWidth = step?.setup ? SETUP_CARD_WIDTH : CARD_WIDTH;

  function clamp(value: number, min: number, max: number): number {
    return Math.min(Math.max(value, min), Math.max(min, max));
  }

  function cardPosition(r: DOMRect | null, width: number): string {
    if (typeof window === "undefined") return "left: 0px; top: 0px;";
    const maxLeft = window.innerWidth - width - VIEWPORT_MARGIN;
    const maxTop = window.innerHeight - cardHeight - VIEWPORT_MARGIN;
    if (!r) {
      const left = clamp(
        (window.innerWidth - width) / 2,
        VIEWPORT_MARGIN,
        maxLeft,
      );
      const top = clamp(
        (window.innerHeight - cardHeight) / 2,
        VIEWPORT_MARGIN,
        maxTop,
      );
      return `left: ${left}px; top: ${top}px;`;
    }

    // Side placement keeps the rest of a wide target visible (a form, a dialog
    // row) while the card talks about one control inside it.
    if (step?.placement === "left" || step?.placement === "right") {
      const rawLeft =
        step.placement === "left" ? r.left - width - GAP : r.right + GAP;
      const left = clamp(rawLeft, VIEWPORT_MARGIN, maxLeft);
      const top = clamp(
        r.top + r.height / 2 - cardHeight / 2,
        VIEWPORT_MARGIN,
        maxTop,
      );
      return `left: ${left}px; top: ${top}px;`;
    }

    const left = clamp(
      r.left + r.width / 2 - width / 2,
      VIEWPORT_MARGIN,
      maxLeft,
    );
    const below = r.bottom + GAP;
    const top =
      below + cardHeight > window.innerHeight - VIEWPORT_MARGIN
        ? clamp(r.top - cardHeight - GAP, VIEWPORT_MARGIN, maxTop)
        : below;
    return `left: ${left}px; top: ${top}px;`;
  }

  $: cardStyle = `width: ${cardWidth}px; pointer-events: auto; ${cardPosition(
    rect,
    cardWidth,
  )}`;

  function handleKeydown(event: KeyboardEvent) {
    if (!$tourState.running) return;
    // Holding an arrow key fires repeats, which would fly through the tour.
    // One press, one step.
    if (event.repeat) return;
    // The first-run setup cards hold the user until setup is done: no Escape.
    if (event.key === "Escape" && !$tourState.setupPending) {
      void closeTour();
    } else if (event.key === "ArrowRight" && $tourState.ready) {
      void nextStep();
    } else if (event.key === "ArrowLeft") {
      void prevStep();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeydown);
  });

  onDestroy(() => {
    window.removeEventListener("keydown", handleKeydown);
  });

  // Re-measure after every paint so the card can flip above a target that is
  // near the bottom of the viewport.
  $: void tick().then(() => {
    if (cardEl) cardHeight = cardEl.offsetHeight || cardHeight;
  });
</script>

{#if $tourState.running && step}
  <div class="tour-root" style="pointer-events: none;">
    {#if rect}
      <div
        class="tour-ring"
        class:no-ring={!$tourState.showRing}
        class:is-pulsing={step.pulseTarget === true}
        style="left: {rect.left - RING_PAD}px; top: {rect.top -
          RING_PAD}px; width: {rect.width +
          RING_PAD * 2}px; height: {rect.height + RING_PAD * 2}px;"
      ></div>
    {:else}
      <div class="tour-scrim"></div>
    {/if}

    <div
      class="tour-card"
      bind:this={cardEl}
      style={cardStyle}
      role="dialog"
      aria-label={step.title}
    >
      <div class="tour-card-title">{step.title}</div>
      <div class="tour-card-scroll">
        {#if step.body}
          {#each step.body.split("\n\n") as paragraph (paragraph)}
            <p class="tour-card-body">{paragraph}</p>
          {/each}
        {/if}
        {#if step.bullets?.length}
          <ul class="tour-card-bullets">
            {#each step.bullets as bullet (bullet)}
              <li>{bullet}</li>
            {/each}
          </ul>
        {/if}
        {#if step.setup}
          <div class="tour-card-setup"><SetupWizard /></div>
        {/if}
      </div>
      {#if $tourState.waitingForTarget}
        <p class="tour-card-hint">Waiting for the app to catch up…</p>
      {/if}
      {#if $tourState.readyTimedOut}
        <p class="tour-card-hint">
          That did not seem to happen - you can carry on anyway.
        </p>
      {/if}
      <div class="tour-card-actions">
        {#if !$tourState.setupPending}
          <button
            class="tour-skip"
            title="Close the tour and do not show it again"
            on:click={() => void closeTour()}>Skip tour</button
          >
        {:else if step.setup}
          <button
            class="tour-skip"
            title="Leave the rest of the setup for later"
            on:click={() => void deferSetup()}>Set up later</button
          >
        {/if}
        <span class="tour-card-progress"
          >{$tourState.index + 1} of {$tourState.total}</span
        >
        <div class="flex gap-2">
          <button
            class="btn btn-sm"
            disabled={$tourState.index === 0}
            on:click={() => void prevStep()}>Back</button
          >
          <button
            class="btn btn-sm primary"
            class:is-pulsing={$tourState.ready && !$tourState.busy}
            disabled={!$tourState.ready}
            on:click={() => void nextStep()}
          >
            {isLastStep ? "Finish" : "Next"}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .tour-root {
    position: fixed;
    inset: 0;
    z-index: 1300;
  }
  .tour-ring {
    position: absolute;
    border-radius: 10px;
    border: 2px solid var(--clr-primary-300);
    /* Outline, not box-shadow: the shadow is the scrim cut-out. */
    outline: 4px solid rgba(53, 132, 228, 0);
    outline-offset: 2px;
    box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.65);
    transition:
      left 0.15s ease,
      top 0.15s ease,
      width 0.15s ease,
      height 0.15s ease;
  }
  /* Flashing ring for a control the user can press. This is the highlight:
     the steady border is dropped so only the flash is drawn. */
  .tour-ring.is-pulsing {
    border-color: transparent;
    animation: ring-pulse 1.4s ease-in-out infinite;
  }
  @keyframes ring-pulse {
    0%,
    100% {
      outline-color: rgba(53, 132, 228, 0);
    }
    50% {
      outline-color: rgba(53, 132, 228, 0.95);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .tour-ring.is-pulsing {
      animation: none;
      outline-color: rgba(53, 132, 228, 0.95);
    }
  }
  .tour-ring.no-ring {
    border-color: transparent;
  }
  .tour-scrim {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
  }
  .tour-card {
    position: absolute;
    display: flex;
    flex-direction: column;
    max-height: calc(100vh - 24px);
    border-radius: 10px;
    padding: 0.9rem 1rem;
    background: var(--clr-surface);
    border: 1px solid var(--adw-border-color);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
  }
  /* Long cards scroll instead of pushing their buttons off screen. */
  .tour-card-scroll {
    overflow-y: auto;
    min-height: 0;
  }
  /* A setup card's fields sit with the copy, inside the same scroll area. */
  .tour-card-setup {
    margin-top: 0.85rem;
  }
  .tour-skip {
    font-size: 0.75rem;
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
    text-decoration: underline;
    color: var(--clr-text-secondary);
    opacity: 0.75;
  }
  .tour-skip:hover {
    opacity: 1;
  }
  .is-pulsing {
    animation: tour-pulse 1.4s ease-in-out infinite;
  }
  @keyframes tour-pulse {
    0%,
    100% {
      box-shadow: 0 0 0 0 rgba(53, 132, 228, 0);
    }
    50% {
      box-shadow: 0 0 0 4px rgba(53, 132, 228, 0.45);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .is-pulsing {
      animation: none;
    }
  }
  .tour-card-title {
    font-weight: 600;
    color: var(--clr-text);
    margin-bottom: 0.35rem;
  }
  .tour-card-body {
    font-size: 0.85rem;
    line-height: 1.4;
    color: var(--clr-text-secondary);
  }
  .tour-card-bullets {
    margin: 0.5rem 0 0;
    padding-left: 1.1rem;
    list-style: disc;
    font-size: 0.8rem;
    line-height: 1.45;
    color: var(--clr-text-secondary);
  }
  .tour-card-bullets li {
    margin-bottom: 0.3rem;
  }
  .tour-card-bullets li:last-child {
    margin-bottom: 0;
  }
  .tour-card-hint {
    font-size: 0.75rem;
    margin-top: 0.4rem;
    color: var(--clr-text-secondary);
    opacity: 0.8;
  }
  .tour-card-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.9rem;
  }
  .tour-card-progress {
    font-size: 0.75rem;
    margin-right: auto;
    color: var(--clr-text-secondary);
  }
</style>
