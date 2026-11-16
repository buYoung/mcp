import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";

export default defineConfig({
    integrations: [
        starlight({
            title: "codemap-search",
            customCss: ["./src/styles/custom.css"],
            components: {
                SocialIcons: "./src/components/SocialIcons.astro",
            },
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
                    label: "Getting Started",
                    translations: { ko: "시작하기" },
                    items: [
                        {
                            slug: "getting-started/installation",
                            label: "Installation",
                            translations: { ko: "설치" },
                        },
                        {
                            slug: "getting-started/client-setup",
                            label: "MCP Client Setup",
                            translations: { ko: "MCP 클라이언트 등록" },
                        },
                    ],
                },
                {
                    label: "Usage",
                    translations: { ko: "사용 방법" },
                    items: [
                        {
                            slug: "usage/tools",
                            label: "MCP Tools",
                            translations: { ko: "탐색 도구 사용" },
                        },
                        {
                            slug: "usage/cli-and-analysis",
                            label: "CLI & Analysis",
                            translations: { ko: "CLI 및 색인·읽기 분석" },
                        },
                        {
                            slug: "usage/languages",
                            label: "Supported Languages & Indexing",
                            translations: { ko: "지원 언어 및 색인 규칙" },
                        },
                    ],
                },
                {
                    label: "Configuration",
                    translations: { ko: "설정" },
                    items: [
                        {
                            slug: "configuration",
                            label: "Configuration Reference",
                            translations: { ko: "설정 레퍼런스" },
                        },
                        {
                            slug: "configuration/pii-redaction",
                            label: "PII Redaction",
                            translations: { ko: "PII 마스킹" },
                        },
                    ],
                },
                {
                    label: "Benchmarks",
                    translations: { ko: "벤치마크" },
                    items: [
                        {
                            slug: "benchmarks",
                            label: "Summary",
                            translations: { ko: "요약" },
                        },
                        {
                            slug: "benchmarks/details",
                            label: "Detailed Results",
                            translations: { ko: "상세 결과" },
                        },
                    ],
                },
            ],
        }),
    ],
});
