import type { BenchmarkPayload, BenchmarkSeries } from "./types";

export const CHART_GEOMETRY = {
    svgW: 460,
    svgH: 260,
    padLeft: 52,
    padRight: 32,
    padTop: 16,
    padBottom: 34,
} as const;

export function computePlotY(val: number, minV: number, maxV: number): number {
    const plotH = CHART_GEOMETRY.svgH - CHART_GEOMETRY.padTop - CHART_GEOMETRY.padBottom;
    return CHART_GEOMETRY.padTop + plotH - ((val - minV) / (maxV - minV)) * plotH;
}

export function getBenchmarkChartModel(locale: "en" | "ko"): {
    envs: string[];
    series: BenchmarkSeries[];
    xCoords: number[];
    accTicks: number[];
    tokTicks: number[];
    titleAcc: string;
    titleTok: string;
    toggleGroupLabel: string;
    payload: BenchmarkPayload;
} {
    const isKo = locale === "ko";
    const envs = ["pi direct", "pi codemode", "Codex code mode"];
    const plotW = CHART_GEOMETRY.svgW - CHART_GEOMETRY.padLeft - CHART_GEOMETRY.padRight;
    const plotH = CHART_GEOMETRY.svgH - CHART_GEOMETRY.padTop - CHART_GEOMETRY.padBottom;
    const xCoords = [
        CHART_GEOMETRY.padLeft + 18,
        CHART_GEOMETRY.padLeft + plotW / 2,
        CHART_GEOMETRY.padLeft + plotW - 18,
    ];

    const series: BenchmarkSeries[] = [
        {
            id: "codemap-search",
            label: "codemap-search",
            color: "var(--sl-color-text-accent)",
            dash: "",
            width: 2.75,
            primary: true,
            acc: [88.9, 89.8, 90.7],
            accDetail: ["88.9% (96/108)", "89.8% (97/108)", "90.7% (98/108)"],
            tok: [946, 829, 1333],
            tokDetail: isKo
                ? ["946k (비캐시 98k)", "829k (비캐시 101k)", "1,333k (비캐시 133k)"]
                : ["946k (uncached 98k)", "829k (uncached 101k)", "1,333k (uncached 133k)"],
        },
        {
            id: "graphify",
            label: "Graphify",
            color: "#0ea5e9",
            dash: "",
            width: 2,
            primary: false,
            acc: [86.1, 81.5, 87.0],
            accDetail: ["86.1% (93/108)", "81.5% (88/108)", "87.0% (94/108)"],
            tok: [1008, 746, 1183],
            tokDetail: isKo
                ? ["1,008k (비캐시 93k)", "746k (비캐시 93k)", "1,183k (비캐시 108k)"]
                : ["1,008k (uncached 93k)", "746k (uncached 93k)", "1,183k (uncached 108k)"],
        },
        {
            id: "zvec-grep",
            label: "zvec-grep",
            color: "#f59e0b",
            dash: "6 3",
            width: 2,
            primary: false,
            acc: [82.4, 84.5, 80.6],
            accDetail: ["82.4% (89/108)", "84.5%* (71/84)", "80.6% (87/108)"],
            tok: [784, 608, 869],
            tokDetail: isKo
                ? ["784k (비캐시 77k)", "608k* (비캐시 79k)", "869k (비캐시 96k)"]
                : ["784k (uncached 77k)", "608k* (uncached 79k)", "869k (uncached 96k)"],
        },
        {
            id: "rg",
            label: isKo ? "rg (기준군)" : "rg (baseline)",
            color: "#10b981",
            dash: "3 3",
            width: 2,
            primary: false,
            acc: [79.6, 80.6, 82.4],
            accDetail: ["79.6% (86/108)", "80.6% (87/108)", "82.4% (89/108)"],
            tok: [555, 505, 866],
            tokDetail: isKo
                ? ["555k (비캐시 68k)", "505k (비캐시 76k)", "866k (비캐시 104k)"]
                : ["555k (uncached 68k)", "505k (uncached 76k)", "866k (uncached 104k)"],
        },
        {
            id: "codebase-memory",
            label: "codebase-memory",
            color: "#8b5cf6",
            dash: "",
            width: 2,
            primary: false,
            acc: [80.6, 77.8, 78.7],
            accDetail: ["80.6% (87/108)", "77.8% (84/108)", "78.7% (85/108)"],
            tok: [1331, 1190, 1827],
            tokDetail: isKo
                ? ["1,331k (비캐시 89k)", "1,190k (비캐시 113k)", "1,827k (비캐시 144k)"]
                : ["1,331k (uncached 89k)", "1,190k (uncached 113k)", "1,827k (uncached 144k)"],
        },
        {
            id: "codegraph",
            label: "CodeGraph",
            color: "#ef4444",
            dash: "8 3",
            width: 2,
            primary: false,
            acc: [72.2, 78.7, 79.6],
            accDetail: ["72.2% (78/108)", "78.7% (85/108)", "79.6% (86/108)"],
            tok: [1727, 1724, 3309],
            tokDetail: isKo
                ? ["1,727k (비캐시 132k)", "1,724k (비캐시 142k)", "3,309k (비캐시 194k)"]
                : ["1,727k (uncached 132k)", "1,724k (uncached 142k)", "3,309k (uncached 194k)"],
        },
    ];

    return {
        envs,
        series,
        xCoords,
        accTicks: [70, 75, 80, 85, 90, 95],
        tokTicks: [0, 1000, 2000, 3000, 3500],
        titleAcc: isKo ? "정답 충족률 (%)" : "Fulfilled rate (%)",
        titleTok: isKo ? "답변당 평균 총 토큰 (k)" : "Mean total tokens / answer (k)",
        toggleGroupLabel: isKo ? "비교 도구 표시 전환" : "Toggle tool series",
        payload: {
            envs,
            series,
            xCoords,
            padTop: CHART_GEOMETRY.padTop,
            plotH,
            padLeft: CHART_GEOMETRY.padLeft,
            svgW: CHART_GEOMETRY.svgW,
        },
    };
}
