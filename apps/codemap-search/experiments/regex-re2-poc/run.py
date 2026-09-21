#!/usr/bin/env python3
"""고정된 원본으로 RE2를 로컬 빌드하고 같은 프로세스에서 Rust와 비교한다."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tarfile
import time
import urllib.request

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
APP = ROOT / 'apps/codemap-search'
CACHE = HERE / '.cache'
SOURCES = {
    're2': ('2025-11-05', 'https://codeload.github.com/google/re2/tar.gz/refs/tags/2025-11-05',
            '87f6029d2f6de8aa023654240a03ada90e876ce9a4676e258dd01ea4c26ffd67'),
    'abseil': ('20250512.1', 'https://codeload.github.com/abseil/abseil-cpp/tar.gz/refs/tags/20250512.1',
               '9b7a064305e9fd94d124ffa6cc358592eb42b5da588fb4e07d09254aa40086db'),
}


def command(arguments, **kwargs):
    print('+', ' '.join(map(str, arguments)), flush=True)
    subprocess.run(list(map(str, arguments)), check=True, **kwargs)


def capture(arguments):
    return subprocess.check_output(arguments, cwd=ROOT, text=True).strip()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def native_build():
    sources = {}
    for name, (_, url, checksum) in SOURCES.items():
        archive = CACHE / (name + '.tar.gz')
        if not archive.exists():
            with urllib.request.urlopen(url, timeout=60) as response:
                archive.write_bytes(response.read())
        if digest(archive.read_bytes()) != checksum:
            raise RuntimeError(f'{name}: SHA-256 mismatch')
        destination = CACHE / name
        if not destination.exists():
            destination.mkdir()
            # Python >=3.12: data filter refuses unsafe paths/links/devices.
            with tarfile.open(archive) as bundle:
                bundle.extractall(destination, filter='data')
        sources[name] = next(path for path in destination.iterdir() if path.is_dir())
    command(['cmake', '-S', HERE / 'native', '-B', CACHE / 'native',
             '-DCMAKE_BUILD_TYPE=Release', f'-DRE2_SOURCE_DIR={sources["re2"]}',
             f'-DABSEIL_SOURCE_DIR={sources["abseil"]}'])
    command(['cmake', '--build', CACHE / 'native', '--parallel', '4'])


def make_input():
    paths = capture(['git', 'ls-files', 'apps', 'packages']).splitlines()
    extensions = {'.rs', '.ts', '.tsx', '.js', '.jsx', '.py', '.go', '.java', '.kt', '.c', '.h', '.cpp'}
    files, manifest = [], []
    for name in sorted(paths):
        path = ROOT / name
        if '/src/' not in name or any('/' + part + '/' in name for part in ('vendor', 'experiments', 'target')):
            continue
        if path.suffix not in extensions:
            continue
        data = path.read_bytes()
        text = data.decode('utf-8')  # Never silently replace invalid bytes.
        files.append({'name': name, 'text': text})
        manifest.append({'path': name, 'bytes': len(data), 'sha256': digest(data)})
    if not files:
        raise RuntimeError('empty repository corpus')
    ascii_text = ''.join(
        f'pub fn process_{i}(input: &str) -> Result<String, Error> {{ // TODO item {i}\n'
        f'  let email = "person{i}@example.com"; let url = "https://example.org/items/{i}";\n'
        '  let authorization = "Bearer POC_ONLY_NOT_A_REAL_CREDENTIAL_123456"; Ok(input.to_string()) }\n'
        for i in range(8192))
    unicode_text = ''.join(f'// 검색 파일 사용자 café αβγ 한글 함수_{i} value{i} １２３ ٤٥٦\n' for i in range(8192))
    corpora = {
        'repository': files,
        'basenames': [{'name': str(i), 'text': Path(f['name']).name} for i, f in enumerate(files)],
        'ascii_generated': [{'name': 'ascii-generated.rs', 'text': ascii_text}],
        'unicode_generated': [{'name': 'unicode-generated.txt', 'text': unicode_text}],
        'long_nonmatch': [{'name': 'long-nonmatch.txt', 'text': 'a' * (2 * 1024 * 1024)}],
    }
    cases = []

    def case(name, pattern, corpus, modes, origin):
        cases.append({'name': name, 'pattern': pattern, 'corpus': corpus, 'modes': modes, 'origin': origin})

    search_modes = ['compile', 'find', 'grep', 'request']
    for name, pattern in [
        ('literal', 'TantivySearchEngine'),
        ('absent_literal', 'codemap_missing_symbol_7f88b30f'),
        ('caller_names', r'\b(?:new|build|parse|search|read|find|detect|resolve|collect|from_str)\b'),
        ('declarations', r'(?m)^[ \t]*(?:pub[ \t]+)?(?:fn|struct|enum|impl)[ \t]+[A-Za-z_][A-Za-z0-9_]*'),
        ('case_insensitive', r'(?i)(?:todo|fixme|error|warning)'),
        ('unicode_literal', r'(?:검색|파일|사용자|성능)'),
    ]:
        case(name, pattern, 'repository', search_modes, 'grep/caller workload; caller_names follows callers/scan.rs')
    case('identifiers', r'[A-Za-z_][A-Za-z0-9_]*', 'repository', ['find'], 'dense match stress')
    case('multiline', r'(?s)struct[ \t]+[A-Za-z_][A-Za-z0-9_]*[^{]*\{.{0,300}?\}',
         'repository', ['compile', 'find'], 'multiline engine workload; no line-search adapter')
    case('basename_filter', r'(?:test|spec|bench|config).*\.(?:rs|ts|tsx|py)$',
         'basenames', ['compile', 'is_match', 'request'], 'tools/find.rs basename-only operation')
    case('conditional', r'^\s*#\s*(if|ifdef|ifndef|elif|elseif|else|endif)\b',
         'ascii_generated', ['compile'], 'events/source_routes/conditional_syntax.rs (compile only)')
    case('bearer_capture', r'\bBearer[ \t]+(?P<secret>[A-Za-z0-9_~+/=.-]{8,})',
         'ascii_generated', ['compile', 'captures', 'request'], 'redact/rules.rs credential.bearer')
    case('assignment_capture', r'''\b(?P<key>[A-Za-z_][A-Za-z0-9_.-]*)["']?[ \t]*(?::[ \t]*(?:&(?:'static[ \t]+)?str|String|string|str))?[ \t]*(?:=|:)[ \t]*''',
         'repository', ['compile', 'captures'], 'redact/text.rs assignments')
    case('unicode_property', r'\p{L}+', 'unicode_generated', ['compile', 'find'], 'shared Unicode letter class')
    case('nested_nonmatch', r'(a+)+b', 'long_nonmatch', ['find'], 'synthetic adverse nonmatch, 2 MiB')
    case('alternation_nonmatch', r'(?:a|aa)+b', 'long_nonmatch', ['find'], 'synthetic adverse nonmatch, 2 MiB')

    catalog = json.loads((APP / 'src/redact/pii/catalog.json').read_text())
    inventory = []
    for definition in catalog:
        flags = '(?ms' + ('i' if definition['is_case_insensitive'] else '') + ')'
        for pattern in definition['patterns']:
            for kind in ('regex', 'field_regex'):
                expression = pattern.get(kind)
                if expression is None:
                    continue
                label = f'{definition["entity"]}/{pattern["name"]}/{kind}'
                for form, value in [('search', expression.replace(r'\b', '')),
                                    ('at_start', r'\A(?P<pii_candidate>' + expression + ')'),
                                    ('after_character', r'\A.(?P<pii_candidate>' + expression + ')')]:
                    inventory.append({'name': label + '/' + form, 'pattern': flags + value})
                if kind == 'regex' and pattern['left_boundary']:
                    inventory.append({'name': label + '/left_boundary',
                                      'pattern': flags + r'\A(?:' + pattern['left_boundary'] + r')\z'})
        if definition['entity'] in ('EMAIL_ADDRESS', 'URL'):
            pattern = flags + definition['patterns'][0]['regex']
            case('pii_' + definition['entity'].lower(), pattern, 'ascii_generated',
                 ['compile', 'captures', 'request'], 'redact/pii/catalog.json, original first pattern and flags')
    probes = [
        ('word_class', r'\w+', 'café 한글 alpha'),
        ('word_boundary', r'\bfoo\b', '한foo글 foo'),
        ('digit_class', r'\d+', '12 １２ ٣٤'),
        ('space_class', r'\s+', 'a\u000bb\u00a0c'),
        ('large_repeat', r'a{1001}', 'a' * 1001),
        ('lookahead', r'foo(?=bar)', 'foobar'),
        ('backreference', r'(a)\1', 'aa'),
        ('lookbehind', r'(?<=fn )[A-Za-z_][A-Za-z0-9_]*', 'pub fn search() {}'),
        ('backtracking_limit', r'^(a|aa)+\1$', 'a' * 32 + '!'),
        ('casefold', r'(?i)k', 'K K k'),
        ('named_optional', r'(?P<first>a)(?P<second>b)?', 'a ab'),
        ('empty_matches', r'a*', 'baaa한a'),
        ('unicode_letters', r'\p{L}+', 'café 한글 αβ'),
        ('redaction_boundary', r'\bBearer[ \t]+(?P<secret>[A-Za-z0-9_~+/=.-]{8,})',
         '한Bearer POC_ONLY_NOT_A_REAL_CREDENTIAL'),
    ]
    stats = {name: {'files': len(values), 'bytes': sum(len(v['text'].encode()) for v in values),
                    'sha256': digest(json.dumps(values, ensure_ascii=False, separators=(',', ':')).encode())}
             for name, values in corpora.items()}
    return {'corpora': corpora, 'cases': cases, 'inventory': inventory,
            'probes': [{'name': n, 'pattern': p, 'text': t} for n, p, t in probes]}, {
        'repository_files': manifest, 'corpora': stats,
        'source_git_commit': capture(['git', 'rev-parse', 'HEAD']),
        'app_cargo_lock_sha256': digest((APP / 'Cargo.lock').read_bytes()),
        'pii_catalog_sha256': digest((APP / 'src/redact/pii/catalog.json').read_bytes()),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--samples', type=int, default=15)
    parser.add_argument('--sample-ms', type=int, default=30)
    parser.add_argument('--seed', type=int, default=42)
    parser.add_argument('--output', type=Path, default=HERE / 'results/run-42.json')
    parser.add_argument('--skip-build', action='store_true')
    parser.add_argument('--reuse-input', action='store_true', help='이전 실행의 코퍼스/패턴 스냅샷 재사용')
    parser.add_argument('--validate-only', action='store_true')
    args = parser.parse_args()
    if args.samples < 3 or args.sample_ms < 1:
        parser.error('--samples >= 3 and --sample-ms >= 1 are required')
    CACHE.mkdir(exist_ok=True)
    if not args.skip_build:
        native_build()
        command(['cargo', 'build', '--release', '--locked', '--manifest-path', HERE / 'Cargo.toml'])
    input_path = CACHE / 'input.json'
    corpus_path = CACHE / 'corpus-metadata.json'
    if args.reuse_input:
        corpus_metadata = json.loads(corpus_path.read_text())
    else:
        inputs, corpus_metadata = make_input()
        input_path.write_text(json.dumps(inputs, ensure_ascii=False))
        corpus_metadata['input_sha256'] = digest(input_path.read_bytes())
        corpus_path.write_text(json.dumps(corpus_metadata, ensure_ascii=False))
    if digest(input_path.read_bytes()) != corpus_metadata['input_sha256']:
        raise RuntimeError('input snapshot SHA-256 mismatch')
    raw_path = CACHE / 'measurement.json'
    started = time.strftime('%Y-%m-%dT%H:%M:%S%z')
    load_before = os.getloadavg()
    arguments = [HERE / 'target/release/regex-re2-poc', input_path, raw_path,
                 args.samples, args.sample_ms, args.seed]
    if args.validate_only:
        arguments.append('--validate-only')
    command(arguments, cwd=ROOT)
    result = json.loads(raw_path.read_text())
    cpu = capture(['sysctl', '-n', 'machdep.cpu.brand_string']) if platform.system() == 'Darwin' else platform.processor()
    result['metadata'] = {
        'started': started, 'finished': time.strftime('%Y-%m-%dT%H:%M:%S%z'),
        'platform': platform.platform(), 'cpu': cpu, 'logical_cpus': os.cpu_count(),
        'load_average_before': load_before, 'load_average_after': os.getloadavg(),
        'rustc': capture(['rustc', '--version']), 'cargo': capture(['cargo', '--version']),
        'cxx': capture(['c++', '--version']), 'cmake': capture(['cmake', '--version']),
        'git_commit': capture(['git', 'rev-parse', 'HEAD']),
        'git_status': capture(['git', 'status', '--short']),
        'cargo_lock_sha256': digest((HERE / 'Cargo.lock').read_bytes()),
        'executable_sha256': digest((HERE / 'target/release/regex-re2-poc').read_bytes()),
        'native_library_sha256': {path.name: digest(path.read_bytes()) for path in (CACHE / 'native').glob('libregex_poc_re2.*') if path.is_file()},
        'build_environment': {key: os.environ.get(key) for key in ('RUSTFLAGS', 'CXXFLAGS', 'CARGO_PROFILE_RELEASE_OPT_LEVEL', 'CARGO_PROFILE_RELEASE_LTO')},
        'native_sources': SOURCES,
        'poc_sources_sha256': {str(path.relative_to(HERE)): digest(path.read_bytes())
                               for path in sorted(HERE.rglob('*')) if path.is_file()
                               and not any(part in ('.cache', 'target', 'results', '__pycache__') for part in path.relative_to(HERE).parts)},
        **corpus_metadata,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    print('Saved:', args.output)


if __name__ == '__main__':
    main()
