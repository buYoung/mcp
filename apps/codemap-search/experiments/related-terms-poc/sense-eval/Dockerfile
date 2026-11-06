# Build from the repository root with this file's allowlisted Docker context.
# Native Linux architecture avoids x86 emulation on ARM hosts.
FROM rust:1.99.1-bookworm AS build
WORKDIR /src/apps/codemap-search
COPY apps/codemap-search/Cargo.toml apps/codemap-search/Cargo.lock apps/codemap-search/build.rs apps/codemap-search/README.md ./
COPY apps/codemap-search/src ./src
COPY apps/codemap-search/vendor ./vendor
COPY apps/codemap-search/queries ./queries
RUN cargo build --locked --release --features related-terms-eval --jobs 2 \
    && rustc --version --verbose > /src/build-info.txt \
    && cargo --version >> /src/build-info.txt

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends python3 git ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/apps/codemap-search/target/release/codemap-search /usr/local/bin/codemap-search
COPY --from=build /src/build-info.txt /src/build-info.txt
COPY apps/codemap-search/experiments/related-terms-poc/term_canonical.py /src/apps/codemap-search/experiments/related-terms-poc/term_canonical.py
COPY apps/codemap-search/experiments/related-terms-poc/sense-eval/*.py /src/apps/codemap-search/experiments/related-terms-poc/sense-eval/
COPY apps/codemap-search/experiments/related-terms-poc/sense-eval/eval-set.json /src/apps/codemap-search/experiments/related-terms-poc/sense-eval/eval-set.json
COPY docs/related-terms/sources.md /src/docs/related-terms/sources.md
ENV PYTHONUNBUFFERED=1 PYTHONDONTWRITEBYTECODE=1
WORKDIR /work
ENTRYPOINT ["python3", "/src/apps/codemap-search/experiments/related-terms-poc/sense-eval/overlay_eval.py"]
CMD ["--binary", "/usr/local/bin/codemap-search", "--dataset", "/dataset/dataset.sqlite3", "--checkouts", "/work/checkouts", "--output-dir", "/work/evaluation", "--clone", "--evidence-timeout-seconds", "3600"]
