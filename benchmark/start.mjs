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
      if (value?.ledger) output.info(`캠페인 상태: ${value.status} · ledger: ${value.ledger}`);
      if (value?.error) output.info(`차단 사유: ${value.error}`);
      if (cancelled || code === 130 || signal === 'SIGINT') {
        const error = new Error(['run', 'resume'].includes(action)
          ? `캠페인을 취소했습니다. ledger: ${value?.ledger ?? 'campaign 디렉터리 확인'}; 자동 재풀이는 하지 않습니다.`
          : '준비를 취소했습니다. pnpm bench:ready로 준비 상태를 다시 확인할 수 있습니다.');
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
  const profile = await prompts.select({
    message: '평가 유형을 선택하세요.',
    choices: Object.entries(catalog.profiles).map(([value, item]) => ({
      name: `${item.name} · ${item.question_ids.length}문항 × ${item.repeats}회`, value,
    })),
  });
  if (catalog.profiles[profile]?.preparation_only !== true) throw new Error('지원하지 않는 벤치 프로필입니다.');
  const solverModel = await prompts.select({
    message: '측정에 사용할 풀이 모델을 선택하세요.',
    default: catalog.default_solver_model,
    choices: catalog.solver_choices.map((item) => ({
      name: `${item.name} / ${item.reasoning_effort}${item.provider ? ` · pi/${item.provider}` : ''}${item.model === catalog.default_solver_model ? ' · 기본값' : ''}`, value: item.model,
    })),
  });
  const targets = catalog.route_targets.map((item) => item.id);
  const plan = await call('prepare', { profile, targets, solver_model: solverModel });
  if (!plan.preparation_only || plan.status !== 'prepared_not_executed') throw new Error('준비 결과의 실행 경계가 올바르지 않습니다.');
  if (plan.spec.model !== solverModel || plan.spec.grader_model !== catalog.grader.model) throw new Error('선택한 풀이 모델 또는 고정 채점 모델이 준비 결과와 다릅니다.');
  output.info(`\n${plan.profile_name}: ${plan.questions}문항 × ${plan.targets.length}개 비교군 × ${plan.repeats}회 = ${plan.runs}회 예정`);
  output.info(`풀이 ${plan.spec.model} / ${plan.spec.reasoning_effort}, 채점 ${plan.spec.grader_model} / ${plan.spec.grader_reasoning_effort}`);
  output.info(`풀이 실행기: ${plan.spec.solver_backend === 'pi' ? `pi ${plan.agent_version ?? ''} (${plan.spec.provider})` : plan.spec.solver_backend === 'claude_code' ? 'Claude Code' : 'Codex'} · 채점 실행기: ${plan.spec.grader_backend}`);
  output.info(`도구 조건: ${plan.targets.map((item) => `${item.name} (${item.tool_condition})`).join(', ')}`);
  output.info(`실제 버전: ${plan.targets.map((item) => `${item.id}=${item.actual_version}`).join(', ')}`);
  output.info(`계획 ID: ${plan.plan_id} · readiness SHA256: ${plan.readiness_sha256}`);
  output.info(`한도: ${plan.spec.limits.elapsed_seconds}초 · 탐색 ${plan.spec.limits.exploration_calls}회 · ${plan.spec.limits.total_tokens.toLocaleString()}토큰(캐시 읽기 제외), 최대 ${plan.spec.concurrency}개 병렬 실행`);
  output.info('응답은 제품 기본값, 색인은 로컬 조건입니다. 문항마다 동일 경로의 조회 전 상태를 복원하도록 준비했습니다.');
  output.info(`준비 기록: ${plan.readiness}`);
  output.info(`준비만으로는 실행하지 않습니다. 승인하면 이 계획에서 최대 ${plan.runs}회 풀이와 해당 답변의 Astra 채점을 수행합니다.`);
  const confirmed = await prompts.confirm({
    message: `계획 ${plan.plan_id} (${solverModel})의 ${plan.runs}개 슬롯 실행을 승인합니까?`, default: false,
  });
  if (!confirmed) return plan;
  if (!plan.readiness_sha256) throw new Error('불변 readiness hash가 없어 실행을 승인할 수 없습니다.');
  return call('run', { plan_id: plan.plan_id, solver_model: solverModel, readiness: plan.readiness,
    readiness_sha256: plan.readiness_sha256, confirmed: true, actor: 'interactive-operator' });
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
