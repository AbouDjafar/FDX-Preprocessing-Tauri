"""Synthetic release checks, run by Actions without building or launching a binary."""
import copy
import hashlib
import json
import os
import runpy
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import promote_tauri as release

VERSION = '2.1.0'
SHA = 'a' * 40
REPOSITORY = 'example/fdx'
RUN = {'id': 123, 'run_attempt': 1, 'status': 'completed', 'conclusion': 'success',
       'event': 'workflow_dispatch', 'path': release.WINDOWS_WORKFLOW,
       'head_repository': {'full_name': REPOSITORY}, 'head_sha': SHA,
       'html_url': 'https://github.com/example/fdx/actions/runs/123'}


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value), encoding='utf-8')


def source_tree(root):
    write_json(root / 'web/package.json', {'version': VERSION})
    write_json(root / 'web/package-lock.json', {'version': VERSION, 'packages': {'': {'version': VERSION}}})
    write_json(root / 'src-tauri/tauri.conf.json', {
        'version': VERSION, 'productName': release.PRODUCT, 'identifier': 'com.fdx.atelier.tauri'})
    (root / 'Cargo.toml').write_text(f'[workspace.package]\nversion = "{VERSION}"\n', encoding='utf-8')
    packages = ('fdx-core', 'fdx-cli', 'fdx-atelier-tauri')
    (root / 'Cargo.lock').write_text(''.join(
        f'[[package]]\nname = "{name}"\nversion = "{VERSION}"\n' for name in packages), encoding='utf-8')


def checksums(folder):
    lines = [hashlib.sha256(path.read_bytes()).hexdigest() + '  ' + path.name
             for path in sorted(folder.iterdir()) if path.name != 'SHA256SUMS.txt']
    (folder / 'SHA256SUMS.txt').write_text('\n'.join(lines) + '\n', encoding='ascii')


def build_assets(folder):
    folder.mkdir(parents=True, exist_ok=True)
    installer = folder / release.installer_name(VERSION)
    installer.write_bytes(b'Synthetic installer fixture, not an executable.')
    manifest = {'schema_version': 1, 'product': release.PRODUCT, 'version': VERSION,
                'commit': SHA, 'repository': REPOSITORY, 'run': '123', 'run_attempt': '1',
                'installer': installer.name, 'installer_sha256': hashlib.sha256(installer.read_bytes()).hexdigest(),
                'verification': sorted(release.VERIFICATIONS)}
    write_json(folder / 'BUILD.json', manifest)
    checksums(folder)


class ManifestTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        source_tree(self.root)

    def test_versions_agree(self):
        self.assertEqual(release.release_version(self.root), VERSION)

    def test_each_manifest_mismatch_is_rejected(self):
        for name in ('web/package.json', 'web/package-lock.json', 'src-tauri/tauri.conf.json'):
            with self.subTest(name=name):
                source_tree(self.root)
                path = self.root / name
                data = release.read_json(path)
                data['version'] = '2.2.0'
                write_json(path, data)
                with self.assertRaises(ValueError):
                    release.release_version(self.root)

    def test_nested_lock_version_mismatch_is_rejected(self):
        path = self.root / 'web/package-lock.json'
        data = release.read_json(path)
        data['packages']['']['version'] = '2.0.0'
        write_json(path, data)
        with self.assertRaisesRegex(ValueError, 'versions must match'):
            release.release_version(self.root)

    def test_cargo_versions_are_checked_without_resolving_dependencies(self):
        for name in ('Cargo.toml', 'Cargo.lock'):
            with self.subTest(name=name):
                source_tree(self.root)
                path = self.root / name
                path.write_text(path.read_text().replace(VERSION, '2.0.0', 1), encoding='utf-8')
                with self.assertRaisesRegex(ValueError, 'versions must match'):
                    release.release_version(self.root)

    def test_incomplete_workspace_lock_is_rejected(self):
        (self.root / 'Cargo.lock').write_text('package = []\n', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'lockfile entries are incomplete'):
            release.release_version(self.root)

    def test_identity_and_product_are_checked(self):
        for key, value in (('identifier', 'com.fdx.renamed'), ('productName', 'FDX Atelier Tauri')):
            with self.subTest(key=key):
                source_tree(self.root)
                path = self.root / 'src-tauri/tauri.conf.json'
                data = release.read_json(path)
                data[key] = value
                write_json(path, data)
                with self.assertRaisesRegex(ValueError, 'identifier mismatch'):
                    release.release_version(self.root)

    def test_invalid_version_is_rejected(self):
        for version in ('02.1.0', 'v2.1.0', '2.1.0-beta', '../2.1.0', None):
            with self.subTest(version=version):
                write_json(self.root / 'web/package.json', {'version': version})
                with self.assertRaisesRegex(ValueError, 'major.minor.patch'):
                    release.release_version(self.root)


class AssetTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.folder = Path(self.temporary.name)
        build_assets(self.folder)

    def verify(self):
        return release.verified_assets(self.folder, REPOSITORY, RUN, VERSION)

    def test_installer_only_release(self):
        self.assertEqual({Path(path).name for path in self.verify()}, {
            'FDX-Atelier-2.1.0-Windows-x64-installation.exe', 'BUILD.json', 'SHA256SUMS.txt'})

    def test_utf8_bom_manifest_is_accepted(self):
        path = self.folder / 'BUILD.json'
        path.write_text(path.read_text(), encoding='utf-8-sig')
        checksums(self.folder)
        self.verify()

    def test_extra_portable_or_directory_is_rejected(self):
        extra = self.folder / 'portable.zip'
        extra.write_bytes(b'not a zip')
        with self.assertRaisesRegex(ValueError, 'exactly one installer'):
            self.verify()
        extra.unlink()
        extra.mkdir()
        with self.assertRaisesRegex(ValueError, 'exactly one installer'):
            self.verify()

    def test_installer_or_manifest_tampering_is_rejected(self):
        for name in (release.installer_name(VERSION), 'BUILD.json'):
            with self.subTest(name=name):
                build_assets(self.folder)
                path = self.folder / name
                path.write_bytes(path.read_bytes() + b' ')
                with self.assertRaisesRegex(ValueError, 'hash verification failed'):
                    self.verify()

    def test_missing_assets_are_rejected(self):
        (self.folder / release.installer_name(VERSION)).unlink()
        with self.assertRaisesRegex(ValueError, 'exactly one installer'):
            self.verify()

    def test_manifest_provenance_is_exact(self):
        changes = {'schema_version': 0, 'product': 'FDX Atelier Tauri', 'version': '2.0.0',
                   'commit': 'b' * 40, 'repository': 'other/fdx', 'run': '124',
                   'run_attempt': '2', 'installer': 'other.exe'}
        for key, value in changes.items():
            with self.subTest(key=key):
                build_assets(self.folder)
                manifest = release.read_json(self.folder / 'BUILD.json')
                manifest[key] = value
                write_json(self.folder / 'BUILD.json', manifest)
                checksums(self.folder)
                with self.assertRaisesRegex(ValueError, 'provenance mismatch'):
                    self.verify()

    def test_missing_or_duplicate_gates_are_rejected(self):
        for gates in ([], ['core'], sorted(release.VERIFICATIONS) + ['core']):
            with self.subTest(gates=gates):
                manifest = release.read_json(self.folder / 'BUILD.json')
                manifest['verification'] = gates
                write_json(self.folder / 'BUILD.json', manifest)
                checksums(self.folder)
                with self.assertRaisesRegex(ValueError, 'gates are incomplete'):
                    self.verify()

    def test_checksum_list_must_be_complete_and_unambiguous(self):
        original = (self.folder / 'SHA256SUMS.txt').read_text()
        first = original.splitlines()[0] + '\n'
        bad_lists = ('', first, original + first, original.upper(),
                     original + '0' * 64 + '  ../escape.exe\n',
                     original + '0' * 64 + '  C:\\escape.exe\n',
                     original + '0' * 64 + '  unknown.exe\n', original + '\n')
        for content in bad_lists:
            with self.subTest(content=content):
                (self.folder / 'SHA256SUMS.txt').write_text(content, encoding='ascii')
                with self.assertRaises(ValueError):
                    self.verify()

    def test_installer_must_match_installation_verified_hash(self):
        (self.folder / release.installer_name(VERSION)).write_bytes(b'replaced after installation test')
        checksums(self.folder)
        with self.assertRaisesRegex(ValueError, 'installation-verified hash'):
            self.verify()


class ProvenanceTests(unittest.TestCase):
    def fake_gh(self, *args, **kwargs):
        if args[0] == 'api':
            if '/actions/runs/' in args[1]:
                return copy.deepcopy(self.run)
            if '/commits/' in args[1]:
                return self.tag_commit
            return {'workflow_runs': copy.deepcopy(self.checks)}
        self.fail(f'Unexpected external call: {args}')

    def setUp(self):
        self.run = copy.deepcopy(RUN)
        self.tag_commit = SHA
        self.checks = [{**copy.deepcopy(RUN), 'path': release.CHECKS_WORKFLOW}]
        self.gh = patch.object(release, 'gh', side_effect=self.fake_gh).start()
        self.addCleanup(patch.stopall)

    def verify(self):
        return release.verified_run(REPOSITORY, '123', f'v{VERSION}', VERSION)

    def test_exact_successful_runs_are_accepted(self):
        self.assertEqual(self.verify(), RUN)

    def test_tag_must_match_manifest_version(self):
        with self.assertRaisesRegex(ValueError, 'tag does not match'):
            release.verified_run(REPOSITORY, '123', 'v2.0.0', VERSION)
        self.gh.assert_not_called()

    def test_wrong_windows_provenance_is_rejected(self):
        for key, value in (('conclusion', 'failure'), ('status', 'in_progress'),
                           ('path', release.CHECKS_WORKFLOW), ('event', 'pull_request'),
                           ('head_repository', {'full_name': 'fork/fdx'}), ('id', 124)):
            with self.subTest(key=key):
                self.run = {**copy.deepcopy(RUN), key: value}
                with self.assertRaisesRegex(ValueError, 'native Windows verification run'):
                    self.verify()

    def test_tag_on_a_newer_commit_is_rejected(self):
        self.tag_commit = 'b' * 40
        with self.assertRaisesRegex(ValueError, 'exact verified commit'):
            self.verify()

    def test_checks_require_exact_sha_repository_and_success(self):
        good = self.checks[0]
        for key, value in (('head_sha', 'b' * 40), ('conclusion', 'failure'),
                           ('head_repository', {'full_name': 'fork/fdx'}),
                           ('path', release.WINDOWS_WORKFLOW), ('status', 'in_progress')):
            with self.subTest(key=key):
                self.checks = [{**good, key: value}]
                with self.assertRaisesRegex(ValueError, 'checks must also pass'):
                    self.verify()

    def test_checks_are_paginated(self):
        good = self.checks[0]
        def paginated(*args, **kwargs):
            if '/workflows/checks.yml/runs' in args[1]:
                return {'workflow_runs': [RUN] * 100 if args[1].endswith('&page=1') else [good]}
            return self.fake_gh(*args, **kwargs)
        self.gh.side_effect = paginated
        self.verify()

    def test_promotion_is_actions_only(self):
        with patch.dict(os.environ, {'GITHUB_ACTIONS': 'false'}):
            with self.assertRaisesRegex(ValueError, 'GitHub Actions'):
                release.promote(Path('.'))
        self.gh.assert_not_called()

    def test_synthetic_promotion_never_rebuilds_or_overwrites(self):
        calls = []
        def simulated(*args, **kwargs):
            calls.append(args)
            if args[:2] == ('run', 'download'):
                build_assets(Path(args[args.index('--dir') + 1]))
                return ''
            if args[:2] == ('release', 'create'):
                self.assertNotIn('--clobber', args)
                self.assertIn('--verify-tag', args)
                self.assertEqual(args[args.index('--title') + 1], 'FDX Atelier 2.1.0')
                self.assertEqual(sum(str(arg).endswith('.exe') for arg in args), 1)
                self.assertFalse(any(str(arg).endswith('.zip') for arg in args))
                return ''
            return self.fake_gh(*args, **kwargs)
        self.gh.side_effect = simulated
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source_tree(root)
            environment = {'GITHUB_ACTIONS': 'true', 'GITHUB_REPOSITORY': REPOSITORY,
                           'RUN_ID': '123', 'TAG': f'v{VERSION}'}
            with patch.dict(os.environ, environment), patch.object(release.subprocess, 'check_output', return_value=SHA + '\n'):
                release.promote(root)
        self.assertEqual(sum(args[:2] == ('release', 'create') for args in calls), 1)


class InstallerResourcesTests(unittest.TestCase):
    def test_license_collection_and_upgrade_hook_use_only_synthetic_dependencies(self):
        script = release.ROOT / 'scripts/tauri_notices.py'
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            destination = root / 'scripts/tauri_notices.py'
            destination.parent.mkdir()
            destination.write_text(script.read_text(encoding='utf-8'), encoding='utf-8')
            rust = root / 'fake-cargo/example'
            rust.mkdir(parents=True)
            (rust / 'LICENSE').write_text('Synthetic Rust license text', encoding='utf-8')
            js = root / 'web/node_modules/example-js'
            write_json(js / 'package.json', {'name': 'example-js', 'version': '1.0.0', 'license': 'MIT'})
            (js / 'LICENSE').write_text('Synthetic JavaScript license text', encoding='utf-8')
            metadata = {'packages': [{'source': 'synthetic-registry', 'name': 'example-rust',
                                      'version': '1.0.0', 'license': 'MIT',
                                      'manifest_path': str(rust / 'Cargo.toml')}]}
            with patch.dict(os.environ, {'GITHUB_ACTIONS': 'true'}), patch.object(
                    release.subprocess, 'check_output', return_value=json.dumps(metadata).encode()) as cargo:
                runpy.run_path(str(destination), run_name='__main__')
            cargo.assert_called_once_with(['cargo', 'metadata', '--locked', '--format-version', '1'], cwd=root.resolve())
            licenses = root / 'qa/licenses'
            self.assertEqual((licenses / 'example-rust-1.0.0/LICENSE').read_text(), 'Synthetic Rust license text')
            self.assertEqual((licenses / 'example-js-1.0.0/LICENSE').read_text(), 'Synthetic JavaScript license text')
            self.assertEqual(len(release.read_json(licenses / 'DEPENDENCIES.json')), 2)
            self.assertFalse((root / 'qa/portable').exists())
            hook = (root / 'qa/installer-hooks.nsh').read_text()
            self.assertIn('NSIS_HOOK_PREINSTALL', hook)
            self.assertIn('/S /UPDATE _?=$R9', hook)
            self.assertNotIn('RMDir /r', hook)


if __name__ == '__main__':
    unittest.main()
