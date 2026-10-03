"""Prepare the single Grafana benchmark profile without model calls."""
import sys

from .route_prepare import main


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        print("준비를 취소했습니다. 준비 로그를 보존합니다.", file=sys.stderr)
        raise SystemExit(130)
