export interface CopyFeedbackMessages {
    copied: string;
    failed: string;
}

export function bindCopyButtons(root: HTMLElement, targetSelector: string, messages: CopyFeedbackMessages): void {
    const buttons = Array.from(root.querySelectorAll<HTMLButtonElement>(".cm-copy-btn"));
    const targetEl = root.querySelector<HTMLElement>(targetSelector);
    if (buttons.length === 0 || !targetEl) return;

    const timers = new WeakMap<HTMLElement, number>();
    let lastAnnouncement = "";

    const report = (button: HTMLButtonElement, ok: boolean) => {
        const tip = button.nextElementSibling as HTMLElement | null;
        if (!tip) return;

        const prev = timers.get(button);
        if (prev) window.clearTimeout(prev);

        button.removeAttribute("data-copied");
        button.removeAttribute("data-failed");

        let message = ok ? messages.copied : messages.failed;
        /* Appending NBSP ensures VoiceOver re-announces identical consecutive status strings */
        if (message === lastAnnouncement) message += "\u00A0";
        lastAnnouncement = message;

        tip.textContent = "";
        void button.offsetWidth;
        button.setAttribute(ok ? "data-copied" : "data-failed", "");
        requestAnimationFrame(() => {
            tip.textContent = message;
        });

        timers.set(
            button,
            window.setTimeout(() => {
                button.removeAttribute("data-copied");
                button.removeAttribute("data-failed");
                tip.textContent = "";
            }, 2000),
        );
    };

    const executeCopy = async (button: HTMLButtonElement) => {
        const text = (targetEl.dataset.copyValue ?? targetEl.textContent ?? "").trim();
        if (!text) return;
        let ok = true;
        try {
            await navigator.clipboard.writeText(text);
        } catch {
            ok = false;
        }
        report(button, ok);
    };

    if ("CommandEvent" in window) {
        targetEl.addEventListener("command", (event) => {
            const cmdEvent = event as Event & { command?: string; source?: Element | null };
            if (cmdEvent.command === "--copy" && cmdEvent.source instanceof HTMLButtonElement) {
                void executeCopy(cmdEvent.source);
            }
        });
    } else {
        for (const btn of buttons) {
            btn.addEventListener("click", () => {
                void executeCopy(btn);
            });
        }
    }
}
