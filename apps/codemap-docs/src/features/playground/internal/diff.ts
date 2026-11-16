import type { DiffLine } from "./types";

export function computeLineDiff(base: string[], curr: string[]): DiffLine[] {
    const m = base.length;
    const n = curr.length;
    const dp: number[][] = Array.from({ length: m + 1 }, () => Array<number>(n + 1).fill(0));
    for (let i = m - 1; i >= 0; i--) {
        for (let j = n - 1; j >= 0; j--) {
            dp[i][j] = base[i] === curr[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
        }
    }
    const out: DiffLine[] = [];
    let i = 0;
    let j = 0;
    let newLine = 1;
    while (i < m && j < n) {
        if (base[i] === curr[j]) {
            out.push({ kind: "same", text: curr[j], lineNo: String(newLine++) });
            i++;
            j++;
        } else if (dp[i + 1][j] >= dp[i][j + 1]) {
            out.push({ kind: "del", text: base[i], lineNo: "−" });
            i++;
        } else {
            out.push({ kind: "add", text: curr[j], lineNo: String(newLine++) });
            j++;
        }
    }
    while (i < m) {
        out.push({ kind: "del", text: base[i++], lineNo: "−" });
    }
    while (j < n) {
        out.push({ kind: "add", text: curr[j++], lineNo: String(newLine++) });
    }
    return out;
}
