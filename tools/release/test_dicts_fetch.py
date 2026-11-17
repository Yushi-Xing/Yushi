#!/usr/bin/env python3
"""上游随包清单更新后，本 fork 继续随包的词库不能丢。"""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('dicts_fetch', Path(__file__).with_name('dicts-fetch.py'))
fetch = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fetch)


class ExtraDictionaryTests(unittest.TestCase):
    def test_updating_official_bundle_preserves_fork_dictionary_and_checksum(self):
        with tempfile.TemporaryDirectory() as directory:
            lock = Path(directory) / 'dicts.lock'
            extra = Path(directory) / 'dicts-extra.lock'
            extra.write_text('general-v1.0.0 = frozen-sha dicts/general.qj\n', encoding='utf-8')
            index = dict(bundled=[dict(id='base', tag='base-v2.0.0', kind='base', qj_format=1)])
            manifest = dict(parts=[dict(name='base.qjz', sha256='new-base-sha')])
            with patch.object(fetch, 'LOCK', lock), patch.object(fetch, 'EXTRA_LOCK', extra), \
                 patch.object(fetch, 'fetch_bytes', side_effect=[json.dumps(index).encode(), json.dumps(manifest).encode()]):
                fetch.update()
                self.assertEqual(fetch.read_lock(), [
                    ('base-v2.0.0', 'new-base-sha', 'dict.qj'),
                    ('general-v1.0.0', 'frozen-sha', 'dicts/general.qj')])
            self.assertEqual(extra.read_text(), 'general-v1.0.0 = frozen-sha dicts/general.qj\n')


if __name__ == '__main__':
    unittest.main()
