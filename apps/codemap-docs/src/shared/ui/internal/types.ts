export type ControlSize = "xs" | "sm" | "md";
export type ButtonVariant = "primary" | "secondary" | "ghost" | "subtle";
export type BadgeVariant = "neutral" | "accent" | "positive" | "negative";

export interface SegmentOption {
    value: string;
    label: string;
    selected?: boolean;
}

export interface ChipOption {
    name: string;
    label: string;
    checked?: boolean;
}

export interface TabItemSpec {
    id: string;
    label: string;
    selected?: boolean;
    hasIndicator?: boolean;
    labelAttr?: string;
}
