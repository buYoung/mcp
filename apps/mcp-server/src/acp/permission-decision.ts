import type { ToolKind } from "@agentclientprotocol/sdk";
import type { PermissionProfile } from "../config/defaults.js";

export const READ_ONLY_TOOL_KINDS: readonly ToolKind[] = ["read", "search", "fetch", "think"];
export const EDIT_TOOL_KINDS: readonly ToolKind[] = [...READ_ONLY_TOOL_KINDS, "edit", "move"];

export const PROFILE_RANK: Record<PermissionProfile, number> = {
    "read-only": 0,
    edit: 1,
    full: 2,
};

export type PermissionDecision = "allow" | "reject";

export function decidePermission(
    profile: PermissionProfile,
    toolKind: ToolKind | null | undefined,
): PermissionDecision {
    if (toolKind == null) {
        // Missing kind → defensive default. permission.md §9 (annotation은 hint, default deny).
        return profile === "full" ? "allow" : "reject";
    }
    if (toolKind === "switch_mode") {
        // Engine's enforce profile is decoupled from agent's self mode.
        return "allow";
    }
    switch (profile) {
        case "read-only":
            return READ_ONLY_TOOL_KINDS.includes(toolKind) ? "allow" : "reject";
        case "edit":
            return EDIT_TOOL_KINDS.includes(toolKind) ? "allow" : "reject";
        case "full":
            // Phase A: no Layer-0 hard block list yet.
            return "allow";
    }
}

export function isProfileDowngrade(from: PermissionProfile, to: PermissionProfile): boolean {
    return PROFILE_RANK[to] < PROFILE_RANK[from];
}
