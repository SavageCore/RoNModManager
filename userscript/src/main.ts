function injectNexusMods() {
  const match = window.location.pathname.match(/^\/readyornot\/mods\/(\d+)/);
  if (!match) return;
  const modId = match[1];

  const checkInterval = setInterval(() => {
    if (document.getElementById("action-ronmm")) {
      clearInterval(checkInterval);
      return;
    }

    const modactions = document.querySelector("ul.modactions");
    if (!modactions) return;
    clearInterval(checkInterval);

    const li = document.createElement("li");
    li.id = "action-ronmm";

    const a = document.createElement("a");
    a.className = "btn inline-flex download-open-tab";
    a.style.cssText = "background-color: var(--theme-primary); border: none;";
    a.href = `ronmm://install/nexus/${modId}`;
    a.tabIndex = 0;
    a.innerHTML = `<span class="flex-label">&#x2B07; Mod Manager</span>`;

    li.appendChild(a);

    const manualBtn = document.getElementById("action-manual");
    if (manualBtn) {
      modactions.insertBefore(li, manualBtn);
    } else {
      modactions.appendChild(li);
    }
  }, 1000);
}

function injectModIo() {
  const injectButton = () => {
    const match = window.location.pathname.match(
      /^\/g\/readyornot\/m\/([^/?#]+)/,
    );
    if (!match) return;
    const modId = match[1];

    const tryInject = () => {
      // Prevent multiple injections
      if (document.getElementById("ronmm-subscribe-btn")) return true;

      const subscribeBtn = document.querySelector('button[large="false"]');
      if (
        subscribeBtn &&
        subscribeBtn.parentElement &&
        !subscribeBtn.classList.contains("tw-opacity-50")
      ) {
        const btn = document.createElement("button") as HTMLButtonElement;
        btn.id = "ronmm-subscribe-btn";
        btn.type = "button";
        btn.innerHTML = "Install via RoN Mod Manager";
        btn.className = Array.from(subscribeBtn.classList).join(" ");
        btn.classList.add(
          "tw-cursor-pointer",
          "hover:tw-bg-primary-hover",
          "focus:tw-bg-primary-hover",
          "hover:tw-border-primary-hover",
          "focus:tw-border-primary-hover",
        );
        btn.classList.remove("tw-opacity-50");
        btn.style.margin = "0.5rem 0 0 0";
        btn.addEventListener("click", () => {
          window.location.href = `ronmm://install/modio/${modId}`;
        });
        subscribeBtn.parentElement.appendChild(btn);
        return true;
      }
      return false;
    };

    // Poll until the hydrated subscribe button is ready
    const pollInterval = setInterval(() => {
      if (tryInject()) {
        clearInterval(pollInterval);
      }
    }, 100);
  };

  // Initial injection
  injectButton();

  // Observe DOM changes for navigation
  const observer = new MutationObserver(() => {
    // Re-extract modId in case of navigation
    const match = window.location.pathname.match(
      /^\/g\/readyornot\/m\/([^/?#]+)/,
    );
    if (!match) return;
    injectButton();
  });
  observer.observe(document.body, { childList: true, subtree: true });
}

const hostname = window.location.hostname;
if (hostname.includes("nexusmods.com")) {
  injectNexusMods();
} else if (hostname.includes("mod.io")) {
  injectModIo();
}
