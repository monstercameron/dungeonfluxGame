import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

MAIN = Path('/Users/earlcameron/Desktop/dungeonflux')
ROOT = MAIN / 'artifacts/worktrees/audit-g01'
SCRATCH = MAIN / 'artifacts/tmp/AUDIT-G01-CLOSURE-001-a2'
EVIDENCE = MAIN / 'development/evidence/dependency-g01/a2/worker'
PYTHON = '/Users/earlcameron/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3'
SCRIPT_REL = 'development/dependency-audit.py'
REPORT_REL = 'development/dependency-audit/report.json'
PINNED = [
    'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
    'crates/df-protocol/Cargo.toml', 'crates/df-rpc-bridge/Cargo.toml',
    'crates/df-observe/Cargo.toml', 'crates/df-tools/Cargo.toml',
    'crates/df-rpc-bridge/vendor/h2/Cargo.toml',
    'development/dependency-audit/metadata/native.json',
    'development/dependency-audit/metadata/wasm.json',
    'development/dependency-audit/packages.json',
]

for child in SCRATCH.iterdir():
    if child.name.startswith('fixture-'):
        if child.is_symlink():
            child.unlink()
        elif child.is_dir():
            shutil.rmtree(child)
EVIDENCE.mkdir(parents=True, exist_ok=True)
source = (ROOT / SCRIPT_REL).read_text()
module_ast = compile(source, SCRIPT_REL, 'exec')


def make_fixture(name):
    fixture = SCRATCH / f'fixture-{name}'
    fixture.mkdir()
    for rel in (*PINNED, REPORT_REL, SCRIPT_REL):
        target = fixture / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / rel, target)
    return fixture


def rebind_fixture(fixture, changed):
    report = fixture / REPORT_REL
    value = json.loads(report.read_text())
    for rel in changed:
        value['input_sha256'][rel] = hashlib.sha256((fixture / rel).read_bytes()).hexdigest()
    report.write_text(json.dumps(value, sort_keys=True))
    current_hash = hashlib.sha256(report.read_bytes()).hexdigest()
    helper = fixture / SCRIPT_REL
    code = helper.read_text()
    old_hash = hashlib.sha256((ROOT / REPORT_REL).read_bytes()).hexdigest()
    helper.write_text(code.replace(old_hash, current_hash))


def run(name, fixture, cwd=None):
    started = time.monotonic()
    try:
        result = subprocess.run(
            [PYTHON, str(fixture / SCRIPT_REL)], cwd=cwd or fixture,
            capture_output=True, text=True, timeout=2,
        )
        return {
            'name': name, 'exit_code': result.returncode,
            'elapsed_seconds': round(time.monotonic() - started, 4),
            'stdout': result.stdout.strip(), 'stderr': result.stderr.strip(),
            'timeout': False, 'passed': result.returncode == 0 if name == 'foreign-cwd-success' else result.returncode != 0,
        }
    except subprocess.TimeoutExpired as error:
        return {
            'name': name, 'exit_code': None,
            'elapsed_seconds': round(time.monotonic() - started, 4),
            'stdout': str(error.stdout or ''), 'stderr': str(error.stderr or ''),
            'timeout': True, 'passed': False,
        }

results = []
base = make_fixture('foreign-cwd')
foreign_cwd = SCRATCH / 'foreign-cwd'; foreign_cwd.mkdir(exist_ok=True)
results.append(run('foreign-cwd-success', base, foreign_cwd))
started = time.monotonic()
try:
    actual = subprocess.run([PYTHON, str(ROOT / SCRIPT_REL)], cwd=foreign_cwd, capture_output=True, text=True, timeout=2)
    results.append({'name': 'actual-checkout-foreign-cwd-success', 'exit_code': actual.returncode, 'elapsed_seconds': round(time.monotonic() - started, 4), 'stdout': actual.stdout.strip(), 'stderr': actual.stderr.strip(), 'timeout': False, 'passed': actual.returncode == 0})
except subprocess.TimeoutExpired:
    results.append({'name': 'actual-checkout-foreign-cwd-success', 'exit_code': None, 'elapsed_seconds': round(time.monotonic() - started, 4), 'timeout': True, 'passed': False})


def rejected(name, change):
    fixture = make_fixture(name)
    change(fixture)
    results.append(run(name, fixture))

rejected('edited-report', lambda f: (f / REPORT_REL).write_text('{}'))
rejected('stale-source-hash', lambda f: (f / 'Cargo.toml').write_bytes((f / 'Cargo.toml').read_bytes() + b'\n'))


def omitted(fixture):
    rel = 'development/dependency-audit/packages.json'
    path = fixture / rel
    value = json.loads(path.read_text())
    value['packages'].pop()
    path.write_text(json.dumps(value))
    rebind_fixture(fixture, [rel])

rejected('omitted-package-identity', omitted)


def malformed(fixture):
    rel = 'development/dependency-audit/metadata/native.json'
    (fixture / rel).write_text('{bad')
    rebind_fixture(fixture, [rel])

rejected('malformed-metadata', malformed)
rejected('missing-input', lambda f: (f / 'Cargo.toml').unlink())


def replace_leaf(fixture, relative, kind):
    path = fixture / relative
    path.unlink()
    if kind == 'fifo':
        os.mkfifo(path)
    elif kind == 'directory':
        path.mkdir()
    elif kind == 'symlink':
        path.symlink_to(ROOT / relative)

for name, rel, kind in (
    ('leaf-source-symlink', 'Cargo.toml', 'symlink'),
    ('leaf-report-symlink', REPORT_REL, 'symlink'),
    ('leaf-metadata-symlink', 'development/dependency-audit/metadata/native.json', 'symlink'),
    ('leaf-packages-symlink', 'development/dependency-audit/packages.json', 'symlink'),
    ('fifo-input', 'Cargo.toml', 'fifo'),
    ('directory-input', 'Cargo.toml', 'directory'),
):
    rejected(name, lambda f, r=rel, k=kind: replace_leaf(f, r, k))


def parent_link(fixture, path_relative, outside_relative):
    inside = fixture / path_relative
    outside = SCRATCH / f'{fixture.name}-outside'
    shutil.copytree(inside, outside)
    shutil.rmtree(inside)
    inside.symlink_to(outside, target_is_directory=True)

rejected('metadata-parent-symlink-outside', lambda f: parent_link(f, 'development/dependency-audit/metadata', 'metadata'))
rejected('report-parent-symlink-outside', lambda f: parent_link(f, 'development/dependency-audit', 'audit'))
rejected('development-parent-symlink-outside', lambda f: parent_link(f, 'development', 'development'))


def crates_parent_link(fixture):
    inside = fixture / 'crates'
    outside = SCRATCH / f'{fixture.name}-outside-crates'
    shutil.copytree(inside, outside)
    shutil.rmtree(inside)
    inside.symlink_to(outside, target_is_directory=True)

rejected('source-parent-symlink-outside', crates_parent_link)


def root_parent_link(fixture):
    link = SCRATCH / f'{fixture.name}-root-link'
    moved = SCRATCH / f'{fixture.name}-root-target'
    fixture.rename(moved)
    link.symlink_to(moved, target_is_directory=True)
    # Runner invokes through the symlink path, making this copy's lexical root the symlink.
    results.append(run('checkout-root-symlink', link))

root_parent_link(make_fixture('checkout-root-symlink'))


def oversized(fixture):
    with (fixture / 'Cargo.toml').open('wb') as stream:
        stream.truncate(32 * 1024 * 1024 + 1)

rejected('oversized-input', oversized)

# In-process repeated calls expose descriptor leaks that subprocess exit would hide.
module_path = ROOT / SCRIPT_REL
spec = importlib.util.spec_from_file_location('audit_a2_candidate', module_path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

def descriptor_count():
    return len(os.listdir('/dev/fd'))

before_success = descriptor_count()
for _ in range(12):
    module.verify()
after_success = descriptor_count()
fd_success = {'case': 'descriptor-cleanup-success', 'before': before_success, 'after': after_success, 'passed': before_success == after_success}
results.append(fd_success)

saved_root = module.ROOT
bad_root = make_fixture('descriptor-refusal')
(bad_root / 'Cargo.toml').unlink()
(bad_root / 'Cargo.toml').symlink_to(ROOT / 'Cargo.toml')
module.ROOT = str(bad_root)
before_refusal = descriptor_count()
for _ in range(12):
    try:
        module.verify()
    except ValueError:
        pass
    else:
        raise AssertionError('symlink refusal unexpectedly succeeded')
after_refusal = descriptor_count()
module.ROOT = saved_root
fd_refusal = {'case': 'descriptor-cleanup-refusal', 'before': before_refusal, 'after': after_refusal, 'passed': before_refusal == after_refusal}
results.append(fd_refusal)

original_dir_fd = os.supports_dir_fd
os.supports_dir_fd = set()
try:
    try:
        module.verify()
    except ValueError as error:
        unsupported = {'case': 'unsupported-dir-fd-capability', 'refused': True, 'reason': str(error), 'passed': 'unsupported' in str(error)}
    else:
        unsupported = {'case': 'unsupported-dir-fd-capability', 'refused': False, 'passed': False}
finally:
    os.supports_dir_fd = original_dir_fd
results.append(unsupported)

capability = {
    'python': sys.version,
    'os_name': os.name,
    'open_supports_dir_fd': os.open in os.supports_dir_fd,
    'flags': {name: getattr(os, name, None) for name in ('O_NOFOLLOW', 'O_DIRECTORY', 'O_NONBLOCK', 'O_CLOEXEC')},
}
(EVIDENCE / 'filesystem-capabilities.json').write_text(json.dumps(capability, indent=2, sort_keys=True) + '\n')
(EVIDENCE / 'verifier-results.json').write_text(json.dumps(results, indent=2, sort_keys=True) + '\n')
if not all(result['passed'] for result in results):
    raise SystemExit(json.dumps(results, indent=2))
print(json.dumps({'cases': len(results), 'all_passed': True, 'capability': capability}, indent=2))
