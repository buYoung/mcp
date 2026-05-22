import { describe, expect, it } from "vitest";
import { decidePermission, isProfileDowngrade } from "../src/acp/permission-decision.js";

describe("decidePermission", () => {
    it("read-only allows read/search/fetch/think kinds", () => {
        for (const kind of ["read", "search", "fetch", "think"] as const) {
            expect(decidePermission("read-only", kind)).toBe("allow");
        }
    });

    it("read-only rejects edit/execute/delete kinds", () => {
        for (const kind of ["edit", "delete", "move", "execute", "other"] as const) {
            expect(decidePermission("read-only", kind)).toBe("reject");
        }
    });

    it("edit allows read + edit + move", () => {
        expect(decidePermission("edit", "read")).toBe("allow");
        expect(decidePermission("edit", "edit")).toBe("allow");
        expect(decidePermission("edit", "move")).toBe("allow");
    });

    it("edit rejects execute and delete", () => {
        expect(decidePermission("edit", "execute")).toBe("reject");
        expect(decidePermission("edit", "delete")).toBe("reject");
    });

    it("full allows every concrete kind (Phase A, no Layer-0 yet)", () => {
        for (const kind of [
            "read",
            "edit",
            "delete",
            "move",
            "search",
            "execute",
            "think",
            "fetch",
            "other",
        ] as const) {
            expect(decidePermission("full", kind)).toBe("allow");
        }
    });

    it("switch_mode is always allowed (decoupled from enforce profile)", () => {
        expect(decidePermission("read-only", "switch_mode")).toBe("allow");
        expect(decidePermission("edit", "switch_mode")).toBe("allow");
        expect(decidePermission("full", "switch_mode")).toBe("allow");
    });

    it("missing toolKind defaults to reject except in full", () => {
        expect(decidePermission("read-only", null)).toBe("reject");
        expect(decidePermission("edit", undefined)).toBe("reject");
        expect(decidePermission("full", null)).toBe("allow");
    });
});

describe("isProfileDowngrade", () => {
    it("recognizes valid downgrades", () => {
        expect(isProfileDowngrade("full", "edit")).toBe(true);
        expect(isProfileDowngrade("full", "read-only")).toBe(true);
        expect(isProfileDowngrade("edit", "read-only")).toBe(true);
    });

    it("rejects same-level and upgrade transitions", () => {
        expect(isProfileDowngrade("read-only", "read-only")).toBe(false);
        expect(isProfileDowngrade("read-only", "edit")).toBe(false);
        expect(isProfileDowngrade("edit", "full")).toBe(false);
    });
});
