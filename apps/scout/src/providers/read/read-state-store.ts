/**
 * read_file의 `file_unchanged` dedup용 프로세스 인메모리 상태 저장소.
 *
 * Claude Code의 `readFileState`를 단순 모사한다(DESIGN §4.1). 직전에 읽은 파일을
 * 절대경로 키로 기록해 두었다가, 같은 `offset`·`limit`로 다시 읽을 때 파일이
 * 그대로면(`Math.floor(mtimeMs)` 동일) 본문 대신 변경 없음 스텁을 반환하게 한다.
 *
 * `timestamp`는 반드시 `Math.floor(mtimeMs)`로 저장한다 — floor를 빼먹으면
 * 부동소수 mtime의 동등 비교가 깨져 dedup이 절대 적중하지 않는다(DESIGN §4.1).
 */
export interface ReadState {
    /** 직전 읽기 시점 파일 mtime을 `Math.floor(mtimeMs)`로 저장한 값. */
    timestamp: number;
    /** 직전 읽기에 적용한 런타임 offset(1-기반). */
    offset: number;
    /** 직전 읽기에 적용한 limit. 미지정이면 undefined. */
    limit: number | undefined;
}

/**
 * 절대경로 → 직전 읽기 상태 맵. 프로세스 메모리에만 존재하며 영속화하지 않는다.
 */
export class ReadStateStore {
    private readonly states = new Map<string, ReadState>();

    /** 절대경로의 직전 읽기 상태를 돌려준다. 없으면 undefined. */
    get(absolutePath: string): ReadState | undefined {
        return this.states.get(absolutePath);
    }

    /** 절대경로의 직전 읽기 상태를 기록한다(덮어쓰기). */
    set(absolutePath: string, state: ReadState): void {
        this.states.set(absolutePath, state);
    }
}
