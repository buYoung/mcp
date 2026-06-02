export function readArguments(argumentsValue: unknown): Record<string, unknown> {
    if (argumentsValue == null) {
        return {};
    }
    if (typeof argumentsValue !== "object" || Array.isArray(argumentsValue)) {
        throw new Error("Expected object arguments.");
    }
    return argumentsValue as Record<string, unknown>;
}

export function readRequiredString(argumentsValue: Record<string, unknown>, key: string): string {
    const value = argumentsValue[key];
    if (typeof value !== "string" || value.trim().length === 0) {
        throw new Error(`Expected non-empty string argument: ${key}`);
    }
    return value;
}

export function readOptionalString(argumentsValue: Record<string, unknown>, key: string): string | undefined {
    const value = argumentsValue[key];
    if (value == null) {
        return undefined;
    }
    if (typeof value !== "string") {
        throw new Error(`Expected string argument: ${key}`);
    }
    return value;
}

export function readOptionalBoolean(argumentsValue: Record<string, unknown>, key: string): boolean | undefined {
    const value = argumentsValue[key];
    if (value == null) {
        return undefined;
    }
    if (typeof value !== "boolean") {
        throw new Error(`Expected boolean argument: ${key}`);
    }
    return value;
}

export function readOptionalInteger(
    argumentsValue: Record<string, unknown>,
    key: string,
    options: { minimum: number },
): number | undefined {
    const value = argumentsValue[key];
    if (value == null) {
        return undefined;
    }
    if (typeof value !== "number" || !Number.isInteger(value) || value < options.minimum) {
        throw new Error(`Expected integer argument >= ${options.minimum}: ${key}`);
    }
    return value;
}

export function readOptionalEnum<Value extends string>(
    argumentsValue: Record<string, unknown>,
    key: string,
    allowedValues: readonly Value[],
): Value | undefined {
    const value = argumentsValue[key];
    if (value == null) {
        return undefined;
    }
    if (typeof value !== "string" || !(allowedValues as readonly string[]).includes(value)) {
        throw new Error(`Expected one of [${allowedValues.join(", ")}] for argument: ${key}`);
    }
    return value as Value;
}
