#!/usr/bin/env python3
"""安装保留检查必须能发现文件删除与字节改写，并拒绝覆盖已有个人文件。"""

from pathlib import Path
import tempfile
import unittest

from windows_package import assert_preferences, seed_preferences


class PreferencesTests(unittest.TestCase):
    def test_preserved_files_pass_and_deleted_learning_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            expected = seed_preferences(folder)
            assert_preferences(folder, expected)
            (folder / 'user-choices.tsv').unlink()
            with self.assertRaises(AssertionError):
                assert_preferences(folder, expected)

    def test_migrated_learning_custom_and_local_downloads_keep_their_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory) / 'roaming'
            local = Path(directory) / 'local'
            expected = seed_preferences(folder)
            for name in expected:
                if name.startswith('user'):
                    destination = folder / 'learning' / name
                elif name.startswith('themes/'):
                    destination = folder / 'custom' / name
                elif name.startswith('models/'):
                    destination = local / 'downloads' / name
                else:
                    continue
                destination.parent.mkdir(parents=True, exist_ok=True)
                (folder / name).rename(destination)
            assert_preferences(folder, expected, local)
            (folder / 'learning/user-preferences.tsv').write_text('changed', encoding='utf-8')
            with self.assertRaises(AssertionError):
                assert_preferences(folder, expected, local)

    def test_config_rewrite_is_detected(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            expected = seed_preferences(folder)
            (folder / 'config.toml').write_text('[general]\n', encoding='utf-8')
            with self.assertRaises(AssertionError):
                assert_preferences(folder, expected)

    def test_seeding_refuses_existing_preferences(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            (folder / 'config.toml').write_text('original', encoding='utf-8')
            with self.assertRaises(RuntimeError):
                seed_preferences(folder)
            self.assertEqual((folder / 'config.toml').read_text(encoding='utf-8'), 'original')


if __name__ == '__main__':
    unittest.main()
