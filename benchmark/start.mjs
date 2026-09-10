import { spawn } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { dirname, resolve } from 'node:path';

const repository = resolve(dirname(fileURLToPath(import.meta.url)), '..');

export function requireTerminal(input = process.stdin, output = process.stdout) {
  if (!input.isTTY || !output.isTTY) throw new Error('bench:start는 입력·출력이 모두 대화형 터미널이어야 합니다.');
  const [major, minor] = process.versions.node.split('.').map(Number);
  if (!(major >= 24 || major === 23 && minor >= 5 || major === 22 && minor >= 13 || major === 20 && minor >= 17)) {
    throw new Error('벤치 CUI는 Node 20.17+, 22.13+ 또는 24+에서 실행하세요.');
  }
}

export function backend(action, request, output = console) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn('python3', ['-m', 'benchmark.cli', action], {
      cwd: repository, stdio: ['pipe', 'pipe', 'pipe'], detached: true,
    });
    let buffer = '';
    let value;
    let failure;
    let cancelled = false;
    const stop = () => {
      if (cancelled) return;
      cancelled = true;
      output.info('취소 요청: 실행 중인 자식 프로세스를 정리하고 결과를 저장합니다.');
      try { process.kill(-child.pid, 'SIGINT'); } catch (error) {
        if (error.code !== 'ESRCH') failure = error.message;
      }
    };
    process.on('SIGINT', stop);
    process.on('SIGTERM', stop);
    child.stdout.setEncoding('utf8');
    child.stdout.on('data', (chunk) => {
      buffer += chunk;
      for (let end; (end = buffer.indexOf('\n')) !== -1;) {
        const line = buffer.slice(0, end);
        buffer = buffer.slice(end + 1);
        if (!line.trim()) continue;
        try {
          const row = JSON.parse(line);
          if (row.event === 'result') value = row.value;
          else if (row.event === 'error') failure = row.message;
          else if (row.event === 'progress') output.info(row.message);
          else failure = `알 수 없는 벤치 응답: ${row.event}`;
        } catch { failure = `벤치 응답 형식 오류: ${line.slice(0, 300)}`; }
      }
    });
    child.stderr.setEncoding('utf8');
    child.stderr.on('data', (chunk) => process.stderr.write(chunk));
    child.stdin.on('error', (error) => { if (error.code !== 'EPIPE') failure = error.message; });
    child.on('error', (error) => { failure = error.message; });
    child.on('close', (code, signal) => {
      process.removeListener('SIGINT', stop);
      process.removeListener('SIGTERM', stop);
      if (value?.report) output.info(`결과표: ${value.report}`);
      if (cancelled || code === 130 || signal === 'SIGINT') {
        const error = new Error('취소했습니다. pnpm bench:start에서 미완료 실행을 이어갈 수 있습니다.');
        error.name = 'BenchCancelled';
        reject(error);
      } else if (code !== 0 || failure || buffer.trim() || value === undefined) {
        reject(new Error(failure || `벤치 실행 실패 (종료 코드 ${code}). 저장된 로그와 결과표를 확인하세요.`));
      } else resolvePromise(value);
    });
    child.stdin.end(JSON.stringify(request));
  });
}

export async function interactive({ prompts, call = backend, output = console }) {
  const catalog = await call('catalog', {});
  if (catalog.pending.length) {
    const run = await prompts.select({
      message: '새 벤치를 시작하거나 미완료 실행을 이어가세요.',
      choices: [{ name: '새 실행', value: null }, ...catalog.pending.map((item) => ({
        name: `${item.id} · ${item.status}`, value: item.id,
      }))],
    });
    if (run) {
      const saved = catalog.pending.find((item) => item.id === run);
      output.info(`이어갈 실행: ${run}. 완료·실패 항목은 재실행하지 않습니다.`);
      if (saved.targets) output.info(`대상: ${saved.targets.join(', ')} · 전체 ${saved.scheduled}회 중 남은 풀이 ${saved.remaining_solver_runs}회`);
      if (saved.spec) output.info(`풀이 ${saved.spec.model} / ${saved.spec.reasoning_effort}, 채점 ${saved.spec.grader_model} / ${saved.spec.grader_reasoning_effort}`);
      if (saved.error) output.warn(`확인이 필요한 이전 오류: ${saved.error}`);
      if (!await prompts.confirm({ message: '남은 풀이·채점을 진행할까요?', default: false })) return;
      return call('resume', { run_id: run, confirmed: true });
    }
  }
  const profile = await prompts.select({
    message: '평가 유형을 선택하세요.',
    choices: Object.entries(catalog.profiles).map(([value, item]) => ({
      name: `${item.name} · ${item.question_ids.length}문항 × ${item.repeats}회`, value,
    })),
  });
  const targets = await prompts.checkbox({
    message: '비교할 대상을 선택하세요. (Space: 선택, Enter: 완료)',
    choices: catalog.targets.map((item) => ({ name: item.name, value: item.id, checked: ['A', 'current'].includes(item.id) })),
    validate: (selected) => selected.length > 0 || '비교 대상을 하나 이상 선택하세요.',
  });
  const plan = await call('prepare', { profile, targets });
  let reuse = null;
  if (plan.reuse_choices.length) {
    reuse = await prompts.select({
      message: '조건이 같은 A 풀이를 재사용할 수 있습니다. 채점은 이번 비교에서 함께 진행합니다.',
      choices: [{ name: 'A 새 측정', value: null }, ...plan.reuse_choices.map((item) => ({
        name: `${item.id} · ${item.runs}회 재사용`, value: item.id,
      }))],
    });
  }
  const reused = reuse ? plan.questions * plan.repeats : 0;
  output.info(`\n${plan.profile_name}: ${plan.questions}문항 × ${plan.repeats}회`);
  output.info(`대상: ${plan.targets.map((item) => `${item.name}${item.source_sha256 ? ` (${item.source_sha256.slice(0, 12)})` : ''}`).join(', ')}`);
  output.info(`예정 ${plan.runs}회 = 새 풀이 ${plan.runs - reused}회 + A 재사용 ${reused}회`);
  output.info(`풀이 ${plan.spec.model} / ${plan.spec.reasoning_effort}, 채점 ${plan.spec.grader_model} / ${plan.spec.grader_reasoning_effort}`);
  output.info(`한도: ${plan.spec.limits.elapsed_seconds}초 · 탐색 ${plan.spec.limits.exploration_calls}회 · ${plan.spec.limits.total_tokens.toLocaleString()}토큰, 동시 실행 ${plan.spec.concurrency}`);
  output.info(`Codex: ${plan.codex_version}, 원자료 저장: ${plan.output_root}`);
  if (!await prompts.confirm({ message: '이 조건으로 풀이와 채점을 시작할까요?', default: false })) {
    output.info('실행하지 않았습니다. 준비된 캐시는 재사용할 수 있습니다.');
    return;
  }
  return call('start', { plan_id: plan.plan_id, reuse, confirmed: true });
}

export async function main() {
  try {
    requireTerminal();
    if (process.argv.length > 2) throw new Error('추가 인자 없이 pnpm bench:start를 실행하세요. 자동 응답은 지원하지 않습니다.');
    const prompts = await import('@inquirer/prompts');
    await interactive({ prompts });
  } catch (error) {
    const cancelled = ['ExitPromptError', 'AbortPromptError', 'BenchCancelled'].includes(error.name);
    console.error(cancelled ? '벤치를 취소했습니다. 저장된 결과는 보존됩니다.' : error.message);
    process.exitCode = cancelled ? 130 : 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) await main();
