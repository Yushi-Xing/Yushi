#!/usr/bin/env python3
"""恢复缓存后必须清掉旧报告与安装器，同时保留 Rust 编译缓存。"""

from pathlib import Path
import tempfile
import unittest

from windows_package import prepare


class OutputTests(unittest.TestCase):
    def test_restored_outputs_cannot_block_or_contaminate_the_next_package(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            previous = ['package-eval/names-static/summary.json', 'package-checks/installer-smoke.json',
                        'windows-package/build-info.json', 'installer/old-setup.exe']
            for name in previous + ['release/deps/keep.rlib', 'i686-pc-windows-msvc/release/keep.dll']:
                path = root / 'target' / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('cached', encoding='utf-8')
            prepare(root)
            for name in previous:
                self.assertFalse((root / 'target' / name).exists(), name)
            self.assertTrue((root / 'target/release/deps/keep.rlib').is_file())
            self.assertTrue((root / 'target/i686-pc-windows-msvc/release/keep.dll').is_file())
            self.assertEqual(list((root / 'target/package-checks').iterdir()), [])
            (root / 'target/package-eval/names-static').mkdir(parents=True, exist_ok=False)

    def test_preparation_also_accepts_an_empty_or_previously_prepared_target(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prepare(root)
            prepare(root)
            self.assertTrue((root / 'target/package-checks').is_dir())


if __name__ == '__main__':
    unittest.main()
