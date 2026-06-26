const biomeExtensionsPattern = "*.{js,jsx,mjs,cjs,ts,tsx,json,jsonc}";
const typeScriptExtensionsPattern = "*.{ts,tsx}";

const quoteFilePath = (filePath) => JSON.stringify(filePath);
const biomeIgnoredPathFragments = [
    "/apps/codemap-search/tests/fixtures/extract/golden/",
    "/apps/codemap-search/tests/fixtures/navigation/",
];

const shouldBiomeCheck = (filePath) => {
    const normalizedPath = filePath.replaceAll("\\", "/");
    if (!normalizedPath.endsWith(".json")) {
        return true;
    }
    return !biomeIgnoredPathFragments.some((fragment) => normalizedPath.includes(fragment));
};

export default {
    [biomeExtensionsPattern]: (filePaths) => {
        const checkedFilePaths = filePaths.filter(shouldBiomeCheck);
        if (checkedFilePaths.length === 0) {
            return [];
        }
        return `biome check --write --no-errors-on-unmatched --files-ignore-unknown=true ${checkedFilePaths
            .map(quoteFilePath)
            .join(" ")}`;
    },
    [typeScriptExtensionsPattern]: () => "pnpm check-types",
};
