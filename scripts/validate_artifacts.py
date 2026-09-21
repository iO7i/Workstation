#!/usr/bin/env python3
"""Validate source-package contracts and reference recipes, NOT compiled Rust behavior.
Python 3.11+; optional jsonschema required for the JSON Schema checks.
Writes a machine-readable report with explicit evidence boundaries.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import tempfile
import time
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]
try:
    import jsonschema
except ImportError as exc:
    raise SystemExit('Install jsonschema in a development environment to run these artifact checks.') from exc


def load(path: str):
    return json.loads((ROOT / path).read_text(encoding='utf-8'))


def connect():
    db = sqlite3.connect(':memory:')
    db.execute('PRAGMA foreign_keys=ON')
    db.executescript((ROOT / 'schema/001_r0.sql').read_text())
    return db


class ArtifactChecks(unittest.TestCase):
    """Tests intentionally do not claim to execute the Rust source."""

    def test_all_json_is_parseable(self):
        for path in ROOT.rglob('*.json'):
            if 'target' not in path.parts:
                json.loads(path.read_text(encoding='utf-8'))

    def test_all_cargo_manifests_parse(self):
        files = list(ROOT.rglob('Cargo.toml'))
        self.assertEqual(len(files), 4)
        for path in files:
            tomllib.loads(path.read_text())

    def test_three_crate_workspace(self):
        cargo = tomllib.loads((ROOT / 'Cargo.toml').read_text())
        self.assertEqual(len(cargo['workspace']['members']), 3)
        for member in cargo['workspace']['members']:
            self.assertTrue((ROOT / member / 'src').is_dir())

    def test_direct_dependency_pins_are_explicit(self):
        deps = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['dependencies']
        for name, spec in deps.items():
            version = spec if isinstance(spec, str) else spec['version']
            self.assertTrue(version.startswith('='), name)

    def test_schemas_are_valid(self):
        for path in (ROOT / 'contracts').glob('*.schema.json'):
            jsonschema.Draft202012Validator.check_schema(json.loads(path.read_text()))

    def test_examples_match_declared_contracts(self):
        for name in ('snapshot', 'share-report', 'envelope'):
            jsonschema.validate(load(f'fixtures/examples/{name}.json'), load(f'contracts/{name}.schema.json'))

    def test_incidents_are_structured_synthetic_inputs(self):
        files = list((ROOT / 'fixtures/incidents').glob('*.json'))
        self.assertEqual(len(files), 6)
        for path in files:
            jsonschema.validate(json.loads(path.read_text()), load('contracts/incident.schema.json'))

    def test_share_contract_rejects_unexpected_sensitive_fields(self):
        report = load('fixtures/examples/share-report.json')
        schema = load('contracts/share-report.schema.json')
        for field in ('path', 'prompt', 'api_key', 'installation_id', 'project_id', 'command_line'):
            bad = dict(report, **{field: 'CANARY_SECRET'})
            with self.assertRaises(jsonschema.ValidationError):
                jsonschema.validate(bad, schema)

    def test_share_example_has_no_private_identifiers(self):
        text = (ROOT / 'fixtures/examples/share-report.json').read_text()
        for value in ('D:\\\\Example', 'codex-example', 'synthetic-installation', 'refs/heads/main'):
            self.assertNotIn(value, text)

    def test_snapshot_contract_rejects_unprotected_process(self):
        snap = load('fixtures/examples/snapshot.json')
        snap['processes']['selected'][0]['disposition'] = 'safe_to_kill'
        with self.assertRaises(jsonschema.ValidationError):
            jsonschema.validate(snap, load('contracts/snapshot.schema.json'))

    def test_snapshot_contract_rejects_offered_repair(self):
        snap = load('fixtures/examples/snapshot.json')
        snap['findings'][0]['repair_available'] = True
        with self.assertRaises(jsonschema.ValidationError):
            jsonschema.validate(snap, load('contracts/snapshot.schema.json'))

    def test_schema_executes_and_integrity_is_ok(self):
        with connect() as db:
            self.assertEqual(db.execute('PRAGMA application_id').fetchone()[0], 1465078832)
            self.assertEqual(db.execute('PRAGMA user_version').fetchone()[0], 1)
            self.assertEqual(db.execute('PRAGMA integrity_check').fetchone()[0], 'ok')

    def test_schema_rejects_invalid_root_kind(self):
        with connect() as db:
            with self.assertRaises(sqlite3.IntegrityError):
                db.execute('INSERT INTO roots VALUES(?,?,?)', ('root', '/example', 'secret_dump'))

    def test_schema_rejects_duplicate_root(self):
        with connect() as db:
            db.execute('INSERT INTO roots VALUES(?,?,?)', ('root', '/example', 'other'))
            with self.assertRaises(sqlite3.IntegrityError):
                db.execute('INSERT INTO roots VALUES(?,?,?)', ('other', '/example', 'other'))

    def test_schema_rejects_malformed_snapshot_json(self):
        with connect() as db:
            with self.assertRaises(sqlite3.IntegrityError):
                db.execute('INSERT INTO scans VALUES(?,?,?)', ('bad', '2026-09-18', '{broken'))

    def test_schema_rejects_oversized_snapshot(self):
        with connect() as db:
            with self.assertRaises(sqlite3.IntegrityError):
                db.execute('INSERT INTO scans VALUES(?,?,?)', ('big', '2026-09-18', json.dumps('x' * 2097152)))

    def test_retention_sql_keeps_twenty_newest(self):
        with connect() as db:
            for n in range(25):
                db.execute('INSERT INTO scans VALUES(?,?,?)', (str(n), f'2026-09-18T12:00:{n:02}.000Z', '{}'))
            db.execute('DELETE FROM scans WHERE run_id NOT IN (SELECT run_id FROM scans ORDER BY observed_at DESC,run_id DESC LIMIT ?)', (20,))
            self.assertEqual(db.execute('SELECT count(*) FROM scans').fetchone()[0], 20)
            self.assertIsNone(db.execute('SELECT run_id FROM scans WHERE run_id=?', ('0',)).fetchone())

    def test_reference_sqlite_backup_restores_metadata(self):
        # This executes Python's SQLite backup API, not rusqlite or Workstation.
        source = connect()
        source.execute('INSERT INTO roots VALUES(?,?,?)', ('root', '/example', 'other'))
        source.commit()
        target = sqlite3.connect(':memory:')
        source.backup(target)
        self.assertEqual(target.execute('SELECT path FROM roots').fetchone()[0], '/example')
        self.assertEqual(target.execute('PRAGMA integrity_check').fetchone()[0], 'ok')
        source.close(); target.close()

    def test_reference_git_recipe_preserves_dirty_ignored_files(self):
        git = shutil.which('git')
        if not git:
            self.skipTest('Git unavailable; reference recipe not executed')
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw); repo = root / 'repo العربية'; wt = root / 'worktree 日本語'
            env = dict(os.environ, GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=os.devnull,
                       GIT_AUTHOR_NAME='Fixture', GIT_AUTHOR_EMAIL='fixture@example.invalid',
                       GIT_COMMITTER_NAME='Fixture', GIT_COMMITTER_EMAIL='fixture@example.invalid')
            def run(*args):
                return subprocess.run([git, *args], env=env, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10).stdout
            run('init', str(repo))
            (repo / 'tracked.txt').write_text('initial')
            (repo / '.gitignore').write_text('.env\n')
            run('-C', str(repo), 'add', '.')
            run('-C', str(repo), 'commit', '-m', 'fixture')
            run('-C', str(repo), 'worktree', 'add', '-b', 'fixture-branch', str(wt))
            (wt / 'tracked.txt').write_text('dirty retained')
            (wt / 'new.txt').write_text('untracked retained')
            (wt / '.env').write_text('SECRET_CANARY=never-echo')
            def digest_tree():
                return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
                        for p in root.rglob('*') if p.is_file()}
            before = digest_tree()
            inspection_env = {k: os.environ[k] for k in ('SystemRoot', 'WINDIR') if k in os.environ}
            inspection_env.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=os.devnull,
                                  GIT_OPTIONAL_LOCKS='0', GIT_TERMINAL_PROMPT='0', GIT_NO_LAZY_FETCH='1',
                                  GIT_PAGER='', GIT_PROTOCOL_FROM_USER='0')
            # Exact argument recipe used by git.rs. No fetch/status/cleanup is performed.
            output = subprocess.run([git, '--no-pager', '--no-optional-locks', '-c', 'core.fsmonitor=false',
                                     '-c', 'core.hooksPath=', '-c', 'protocol.allow=never',
                                     'worktree', 'list', '--porcelain', '-z'], cwd=repo, env=inspection_env,
                                    check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5).stdout
            self.assertTrue(output.endswith(b'\0'))
            self.assertIn(str(wt).encode(), output)
            self.assertNotIn(b'SECRET_CANARY', output)
            self.assertEqual(before, digest_tree())

    def test_no_future_privileged_or_secret_commands_in_cli(self):
        source = (ROOT / 'crates/workstation-cli/src/main.rs').read_text()
        for forbidden in ('RepairAll', 'SecretReveal', 'FactoryReset', 'UnregisterWsl'):
            self.assertNotIn(forbidden, source)
        platform_source = '\n'.join(p.read_text() for p in (ROOT / 'crates/workstation-platform/src').glob('*.rs'))
        for forbidden in ('taskkill', 'wsl --shutdown', 'docker system prune', 'CryptUnprotectData', 'remove_dir_all'):
            self.assertNotIn(forbidden, platform_source)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, default=ROOT / 'validation/artifact-checks.json')
    args = parser.parse_args()
    started = time.monotonic()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(ArtifactChecks)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    report = {
        'schema_version': 1,
        'kind': 'executed_source_artifact_and_reference_recipe_checks',
        'tests_run': result.testsRun,
        'failures': len(result.failures), 'errors': len(result.errors),
        'skipped': len(result.skipped), 'elapsed_seconds': round(time.monotonic() - started, 3),
        'python_sqlite_version': sqlite3.sqlite_version,
        'git_version': subprocess.run(['git', '--version'], capture_output=True, text=True).stdout.strip() if shutil.which('git') else None,
        'rust_compilation_executed': False,
        'rust_tests_executed': False,
        'windows_tests_executed': False,
        'limitations': [
            'JSON Schema examples and SQL recipes were validated independently, not through the Rust binary.',
            'The host Python SQLite engine is not the bundled Rust SQLite engine.',
            'The Git command recipe was exercised on temporary Linux fixtures, not on Windows.',
            'Static source checks do not establish Rust compilation or runtime correctness.'
        ],
        'failure_details': [detail for _, detail in result.failures + result.errors]
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    raise SystemExit(0 if result.wasSuccessful() else 1)
