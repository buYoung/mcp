import { get } from "node:http";

export interface HttpResponse {
    statusCode: number;
    body: string;
}

/**
 * Minimal localhost HTTP GET on top of `node:http` — avoids depending on the DOM
 * `fetch` typings, and gives explicit timeout control for webserver polling.
 */
export function httpGet(url: string, timeoutMs: number): Promise<HttpResponse> {
    return new Promise<HttpResponse>((resolve, reject) => {
        const request = get(url, (response) => {
            const chunks: Buffer[] = [];
            response.on("data", (chunk: Buffer) => {
                chunks.push(chunk);
            });
            response.on("end", () => {
                resolve({
                    statusCode: response.statusCode ?? 0,
                    body: Buffer.concat(chunks).toString("utf8"),
                });
            });
        });
        request.setTimeout(timeoutMs, () => {
            request.destroy(new Error(`HTTP request timed out after ${timeoutMs}ms: ${url}`));
        });
        request.on("error", (error) => {
            reject(error);
        });
    });
}
