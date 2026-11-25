import contextlib
import importlib.util
import io
import json
import pathlib
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('catalog', pathlib.Path(__file__).with_name('i18n-catalog.py'))
catalog = importlib.util.module_from_spec(spec)
spec.loader.exec_module(catalog)
pack_spec = importlib.util.spec_from_file_location('pack', pathlib.Path(__file__).with_name('build-language-pack.py'))
pack = importlib.util.module_from_spec(pack_spec)
pack_spec.loader.exec_module(pack)


class CatalogTests(unittest.TestCase):
    def test_rust_unicode_and_format_fields(self):
        self.assertEqual(catalog.decode(r'"Hello \u{00B7} world"'), 'Hello · world')
        self.assertEqual(catalog.fields('Value: {n}, {} {{ok}}'), ['{n}', '{}'])

    def test_pack_has_versioned_header_and_rejects_invalid_placeholders(self):
        data = {'language': 'pt-BR', 'messages': {'Hello {}': 'Olá {}'}, 'needs_review': {}}
        packed = pack.build(data)
        self.assertTrue(packed.startswith(b'WRLANG\x01\0'))
        self.assertIn('Olá {}'.encode('utf-8'), packed)
        data['messages']['Hello {}'] = 'Olá {name}'
        with self.assertRaises(ValueError): pack.build(data)
        data['needs_review'] = {'Open': ['Abrir', 'Aberto']}
        with self.assertRaises(ValueError): pack.build(data)

    def test_collects_translations_conflicts_and_missing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / 'test.rs').write_text('pt!("r", "Open", "Abrir"); pt!("r", "Open", "Aberto"); t!("r", "Close");', encoding='utf-8')
            values, required, invalid = catalog.extract(root)
            self.assertEqual(values['Open'], {'Abrir', 'Aberto'})
            self.assertEqual(required, {'Open', 'Close'})
            self.assertEqual(invalid, [])

    def test_export_keeps_catalog_language(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / 'test.rs').write_text('pt!("r", "Open", "Abrir"); t!("r", "Close");', encoding='utf-8')
            target = root / 'ja.json'
            target.write_text('{"language": "ja", "messages": {"Close": "閉じる"}}', encoding='utf-8')
            argv = sys.argv
            sys.argv = ['i18n-catalog.py', 'export', '--source', str(root), '--catalog', str(target)]
            try:
                with contextlib.redirect_stdout(io.StringIO()):
                    catalog.main()
            finally:
                sys.argv = argv
            result = json.loads(target.read_text(encoding='utf-8'))
            self.assertEqual(result['language'], 'ja')
            self.assertEqual(result['messages'], {'Close': '閉じる'})

    def test_export_preserves_unresolved_conflicts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / 'test.rs').write_text('t!("r", "Open");', encoding='utf-8')
            target = root / 'pt-BR.json'
            target.write_text(json.dumps({'language': 'pt-BR', 'messages': {}, 'needs_review': {'Open': ['Abrir', 'Aberto']}}), encoding='utf-8')
            argv = sys.argv
            sys.argv = ['i18n-catalog.py', 'export', '--source', str(root), '--catalog', str(target)]
            try:
                with contextlib.redirect_stdout(io.StringIO()): catalog.main()
            finally:
                sys.argv = argv
            self.assertEqual(json.loads(target.read_text(encoding='utf-8'))['needs_review'], {'Open': ['Abrir', 'Aberto']})


if __name__ == '__main__':
    unittest.main()
