function stripDuplicateIds(root: ParentNode): void {
    for (const el of Array.from(root.querySelectorAll("[id]"))) {
        el.removeAttribute("id");
    }
}

export function extractPeekContent(container: Element, hash: string): { title: string; frag: DocumentFragment } | null {
    const rawId = decodeURIComponent(hash.slice(1));
    const target = container.querySelector<HTMLElement>(`#${CSS.escape(rawId)}`);
    if (!target) return null;

    const frag = document.createDocumentFragment();

    if (target instanceof HTMLDetailsElement) {
        const summary = target.querySelector(":scope > summary");
        const title = (summary?.textContent ?? rawId).trim();
        for (const child of Array.from(target.children)) {
            if (child === summary) continue;
            const clone = child.cloneNode(true) as HTMLElement;
            stripDuplicateIds(clone);
            frag.append(clone);
        }
        return { title, frag };
    }

    if (target.hasAttribute("data-peek-block")) {
        const title = target.getAttribute("data-peek-title") ?? rawId;
        const clone = target.cloneNode(true) as HTMLElement;
        clone.removeAttribute("id");
        stripDuplicateIds(clone);
        frag.append(clone);
        return { title, frag };
    }

    const headingWrapper = target.closest(".sl-heading-wrapper") ?? target;
    const tagMatch = /^H([1-6])$/.exec(target.tagName);
    const baseLevel = tagMatch ? Number(tagMatch[1]) : 3;
    const title = (target.textContent ?? "").trim();

    let cursor = headingWrapper.nextElementSibling;
    while (cursor) {
        const innerHeading = cursor.matches("h1, h2, h3, h4, h5, h6")
            ? cursor
            : cursor.querySelector(":scope > h1, :scope > h2, :scope > h3, :scope > h4, :scope > h5, :scope > h6");
        if (innerHeading) {
            const m = /^H([1-6])$/.exec(innerHeading.tagName);
            const nextLevel = m ? Number(m[1]) : 6;
            if (nextLevel <= baseLevel) break;
        }
        const clone = cursor.cloneNode(true) as HTMLElement;
        stripDuplicateIds(clone);
        frag.append(clone);
        cursor = cursor.nextElementSibling;
    }
    return { title, frag };
}
