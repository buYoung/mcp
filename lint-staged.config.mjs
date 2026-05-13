const biomeExtensionsPattern = "*.{js,jsx,mjs,cjs,ts,tsx,json,jsonc}";
const typeScriptExtensionsPattern = "*.{ts,tsx}";

const quoteFilePath = (filePath) => JSON.stringify(filePath);

export default {
    [biomeExtensionsPattern]: (filePaths) =>
        `biome check --write --no-errors-on-unmatched --files-ignore-unknown=true ${filePaths
            .map(quoteFilePath)
            .join(" ")}`,
    [typeScriptExtensionsPattern]: () => "pnpm check-types",
};
