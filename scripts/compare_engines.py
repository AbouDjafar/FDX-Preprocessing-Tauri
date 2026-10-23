"""CI-only differential checks using generated data and the retained Python reference."""
import hashlib
import json
import subprocess
import sys
import tempfile
from datetime import date, datetime
from decimal import Decimal
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'src'))
sys.path.insert(0, str(ROOT / 'tests'))
from conftest import project
from fdx_atelier.engine import analyze
from fdx_atelier.models import Decision, FieldRule, cell_key
from openpyxl import load_workbook


def comparable(value):
    if value is None or value == '':
        return ('empty', '')
    if isinstance(value, bool):
        return ('boolean', str(value).lower())
    if isinstance(value, (date, datetime)):
        return ('date', value.strftime('%Y-%m-%d'))
    if isinstance(value, (int, float, Decimal)):
        return ('number', Decimal(str(value)))
    return ('text', value)


def typed(value):
    kind = value['kind']
    raw = value['value']
    if kind in ('integer', 'decimal'):
        return ('number', Decimal(raw))
    if kind == 'empty' or raw == '':
        return ('empty', '')
    return (kind, raw)


def invoke(binary, *args):
    run = subprocess.run([str(binary), *map(str, args)], capture_output=True, text=True, encoding='utf-8')
    if run.returncode:
        raise AssertionError(f'CLI {args[0]} failed: {run.stderr}')
    return run.stdout


def check(binary, folder, policy):
    folder.mkdir()
    config = project.__wrapped__(folder)
    config.rules['TEL'] = FieldRule(phone_policy=policy)
    baseline = hashlib.sha256(Path(config.source_path).read_bytes()).hexdigest()
    reference = analyze(config)
    config_file = folder / 'config.json'
    config_file.write_text(config.model_dump_json(), encoding='utf-8')
    result_file = folder / 'analysis.json'
    invoke(binary, 'analyze', '--config', config_file, '--output', result_file)
    actual = json.loads(result_file.read_text(encoding='utf-8'))
    assert actual['row_count'] == reference.row_count
    expected_changes = {change.key: comparable(change.after) for change in reference.changes}
    actual_changes = {change['key']: typed(change['after']) for change in actual['changes']}
    assert actual_changes == expected_changes, (policy, expected_changes, actual_changes)
    expected_findings = {(f.key, f.kind) for f in reference.findings}
    actual_findings = {(f['key'], f['kind']) for f in actual['findings']}
    assert actual_findings == expected_findings, (policy, expected_findings, actual_findings)
    target = folder / 'output.xlsx'
    invoke(binary, 'export', '--config', config_file, '--output', target, '--allow-issues')
    book = load_workbook(target, data_only=False)
    assert book['Données']['F3'].value == '=SUM(C2:C3)'
    assert book['Données']['E2'].value == '00123'
    assert book['Données'].freeze_panes == 'A2'
    assert book['Données'].auto_filter.ref == 'A1:G3'
    assert book['Conserver']['A2'].value == 12345.67
    for sheet in book:
        for row in sheet:
            for cell in row:
                assert not cell.alignment.textRotation, (sheet.title, cell.coordinate)
                if isinstance(cell.value, (int, float)):
                    assert '#,##' not in cell.number_format
    assert book['À vérifier'].max_row - 1 == len(actual['findings'])
    book.close()
    assert hashlib.sha256(Path(config.source_path).read_bytes()).hexdigest() == baseline
    return {'policy': policy, 'rows': actual['row_count'], 'changes': len(actual_changes), 'findings': len(actual_findings)}


if __name__ == '__main__':
    binary = Path(sys.argv[1]).resolve()
    with tempfile.TemporaryDirectory(prefix='fdx-oracle-') as temporary:
        results = [check(binary, Path(temporary) / policy, policy) for policy in ('report', 'first', 'all', 'clear')]
    destination = ROOT / 'qa' / 'parity.json'
    destination.parent.mkdir(exist_ok=True)
    destination.write_text(json.dumps({'ok': True, 'cases': results}, indent=2), encoding='utf-8')
    print(json.dumps({'ok': True, 'cases': len(results)}))
