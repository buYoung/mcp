#!/usr/bin/env python3
"""축 A 스케일링/성능 측정 하베스 — codemap-search.
인덱싱(N=5 wall+peak RSS), 인덱스크기, 파일/심볼, startup, MCP warm 도구지연(p50/p90/p95)."""
import subprocess, json, os, re, shutil, statistics, sys
from time import perf_counter

BIN = "/Users/buyong/workspace/private/buyong-mcp/apps/codemap-search/target/release/codemap-search"
ROOT = "/Users/buyong/workspace/private/codemap-bench/corpora"
TIME = "/usr/bin/time"
N_INDEX, N_START, WARM, N_QUERY = 5, 12, 3, 25

# name, dir, lang, search_q, grep_pat, find_pat, read_file, loc, files
CORPORA = [
    ("fd",       "fd",       "Rust",       "walk directory", "fn main",         "**/*.rs", "Cargo.toml",   6813,   23),
    ("ripgrep",  "ripgrep",  "Rust",       "search matcher", "fn main",         "**/*.rs", "Cargo.toml",   39070,  102),
    ("scrapy",   "scrapy",   "Python",     "spider request", "def parse",       "**/*.py", "setup.py",     63513,  439),
    ("vue-core", "vue-core", "TypeScript", "reactive effect","export function", "**/*.ts", "package.json", 128285, 519),
]

def pct(xs, p):
    xs = sorted(xs); k = (len(xs)-1)*p/100; f = int(k)
    return xs[f] if f+1 >= len(xs) else xs[f]+(xs[f+1]-xs[f])*(k-f)

def parse_time_l(stderr):
    real = re.search(r"([\d.]+)\s+real", stderr)
    rss  = re.search(r"(\d+)\s+maximum resident set size", stderr)
    return (float(real.group(1)) if real else None,
            int(rss.group(1)) if rss else None)

def measure_index(cwd):
    walls, rsses = [], []
    for _ in range(N_INDEX):
        shutil.rmtree(os.path.join(cwd, ".codemap"), ignore_errors=True)
        p = subprocess.run([TIME, "-l", BIN, "index", "."], cwd=cwd,
                           capture_output=True, text=True)
        w, r = parse_time_l(p.stderr)
        if w is not None: walls.append(w)
        if r is not None: rsses.append(r)
    return walls, rsses

def measure_startup():
    xs = []
    for _ in range(N_START):
        t0 = perf_counter()
        subprocess.run([BIN, "--version"], capture_output=True)
        xs.append((perf_counter()-t0)*1000)
    return xs

def codemap_counts(cwd):
    p = subprocess.run([BIN, "codemap"], cwd=cwd, capture_output=True, text=True)
    f = re.search(r"Total Files\*\*:\s*([\d,]+)", p.stdout)
    s = re.search(r"Total Symbols\*\*:\s*([\d,]+)", p.stdout)
    g = lambda m: int(m.group(1).replace(",","")) if m else None
    return g(f), g(s)

class Mcp:
    def __init__(self, cwd):
        self.p = subprocess.Popen([BIN, "mcp"], cwd=cwd, stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
        self._id = 0
    def call(self, method, params):
        self._id += 1; mid = self._id
        req = json.dumps({"jsonrpc":"2.0","id":mid,"method":method,"params":params})
        t0 = perf_counter()
        self.p.stdin.write(req+"\n"); self.p.stdin.flush()
        resp = None
        while True:
            line = self.p.stdout.readline()
            if not line: break
            try: m = json.loads(line)
            except Exception: continue
            if m.get("id") == mid: resp = m; break
        return (perf_counter()-t0)*1000, resp
    def tool(self, name, args):
        return self.call("tools/call", {"name":name, "arguments":args})
    def close(self):
        try: self.p.stdin.close(); self.p.terminate()
        except Exception: pass

def resp_len(resp):
    try: return len(resp["result"]["content"][0]["text"])
    except Exception: return None

def measure_tools(cwd, q, grep_pat, find_pat, read_file):
    m = Mcp(cwd)
    m.call("initialize", {"protocolVersion":"2024-11-05","capabilities":{},
                          "clientInfo":{"name":"bench","version":"0"}})
    tools = {
        "overview": ("overview", {}),
        "search":   ("search",   {"query": q}),
        "read":     ("read",     {"file_path": read_file}),
        "find":     ("find",     {"pattern": find_pat}),
        "grep":     ("grep",     {"pattern": grep_pat, "output_mode":"files_with_matches"}),
    }
    out = {}
    for label,(name,args) in tools.items():
        for _ in range(WARM): m.tool(name, args)
        samples, last = [], None
        for _ in range(N_QUERY):
            ms, resp = m.tool(name, args); samples.append(ms); last = resp
        out[label] = {"p50":pct(samples,50),"p90":pct(samples,90),
                      "p95":pct(samples,95),"min":min(samples),
                      "resp_chars":resp_len(last)}
    m.close()
    return out

def main():
    results = []
    print("startup baseline 측정...", file=sys.stderr)
    su = measure_startup()
    startup_p50 = pct(su,50)
    for name,d,lang,q,gp,fp,rf,loc,files in CORPORA:
        cwd = os.path.join(ROOT, d)
        print(f"[{name}] indexing...", file=sys.stderr)
        walls, rsses = measure_index(cwd)
        size_kb = int(subprocess.run(["du","-sk", os.path.join(cwd,".codemap/index")],
                       capture_output=True, text=True).stdout.split()[0])
        tf, tsym = codemap_counts(cwd)
        print(f"[{name}] tool latency (MCP warm)...", file=sys.stderr)
        tools = measure_tools(cwd, q, gp, fp, rf)
        med_wall = statistics.median(walls)
        results.append({
            "corpus":name,"lang":lang,"loc":loc,"files_tokei":files,
            "index_files":tf,"symbols":tsym,
            "index_wall_s":{"median":med_wall,"min":min(walls),"max":max(walls)},
            "peak_rss_mb":round(statistics.median(rsses)/1048576,1) if rsses else None,
            "index_size_mb":round(size_kb/1024,2),
            "throughput_loc_per_s":int(loc/med_wall),
            "throughput_files_per_s":int(tf/med_wall) if tf else None,
            "tools_ms":tools,
        })
        shutil.rmtree(os.path.join(cwd, ".codemap"), ignore_errors=True)
    final = {"startup_ms_p50":round(startup_p50,1),
             "config":{"N_INDEX":N_INDEX,"N_QUERY":N_QUERY,"WARM":WARM},
             "results":results}
    out_path = "/Users/buyong/workspace/private/codemap-bench/artifacts/axisA-results.json"
    with open(out_path,"w") as fh: json.dump(final, fh, indent=2)
    print(json.dumps(final, indent=2))
    print(f"\nwritten: {out_path}", file=sys.stderr)

if __name__ == "__main__":
    main()
