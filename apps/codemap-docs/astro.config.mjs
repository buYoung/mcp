import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";

export default defineConfig({
    integrations: [
        starlight({
            title: "codemap-search",
            defaultLocale: "root",
            locales: {
                root: {
                    label: "English",
                    lang: "en",
                },
                ko: {
                    label: "한국어",
                    lang: "ko",
                },
            },
            social: [
                {
                    icon: "github",
                    label: "GitHub",
                    href: "https://github.com/buYoung/mcp",
                },
            ],
            sidebar: [
                {
                    label: "Benchmarks",
                    translations: { ko: "벤치마크" },
                    items: [
                        {
                            slug: "benchmarks/why-benchmark",
                            label: "Why Benchmark",
                            translations: { ko: "왜 측정했는가" },
                        },
                        {
                            slug: "benchmarks/methodology",
                            label: "Methodology",
                            translations: { ko: "측정 방법" },
                        },
                        {
                            slug: "benchmarks/metrics",
                            label: "Metrics",
                            translations: { ko: "측정 지표" },
                        },
                        {
                            slug: "benchmarks/results",
                            label: "Results",
                            translations: { ko: "측정 결과" },
                        },
                        {
                            slug: "benchmarks/what-changed",
                            label: "What Changed Because of Benchmarks",
                            translations: { ko: "벤치마크로 바뀐 것" },
                        },
                        {
                            slug: "benchmarks/limits",
                            label: "Limits",
                            translations: { ko: "한계와 비주장 범위" },
                        },
                        {
                            slug: "benchmarks/reproduction-notes",
                            label: "Reproduction Notes",
                            translations: { ko: "재현 조건과 참고 사항" },
                        },
                    ],
                },
            ],
        }),
    ],
});
