"""Exercise the shipped Windows binary over its real Tauri bridge, never a mock."""
import hashlib
import json
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

from openpyxl import load_workbook
from selenium import webdriver
from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait
from selenium.webdriver.support import expected_conditions as EC


class TauriOptions(webdriver.ChromeOptions):
    def to_capabilities(self):
        return {'tauri:options': {'application': str(Path(sys.argv[1]).resolve())}}


def call(driver, op, payload=None, session=None, fail=False):
    request = {'op': op, 'task_id': str(uuid.uuid4()), 'payload': payload or {}}
    if session:
        request.update(session_id=session['id'], revision=session['revision'])
    response = driver.execute_async_script('''
      const done = arguments[arguments.length - 1];
      window.__TAURI_INTERNALS__.invoke('fdx', { request: arguments[0], onEvent: null })
        .then(value => done({value})).catch(error => done({error}));
    ''', request)
    if fail:
        assert 'error' in response, (op, response)
        return response['error']
    assert 'error' not in response, (op, response)
    return response['value']


def check(driver, folder):
    wait = WebDriverWait(driver, 30)
    wait.until(lambda d: 'FDX' in d.find_element(By.TAG_NAME, 'body').text)
    assert driver.execute_script('return getComputedStyle(document.body).backgroundColor') != 'rgb(255, 255, 255)'
    driver.set_window_size(1366, 768)
    wait.until(EC.element_to_be_clickable((By.CSS_SELECTOR, '[data-testid="try-demo"]')))
    driver.save_screenshot(str(folder / 'native-start.png'))
    driver.find_element(By.CSS_SELECTOR, '[data-testid="try-demo"]').click()
    wait.until(EC.element_to_be_clickable((By.CSS_SELECTOR, '[data-testid="stage-association"]'))).click()
    confirm = '//button[normalize-space()="Confirmer toutes les associations"]'
    wait.until(EC.element_to_be_clickable((By.XPATH, confirm))).click()
    wait.until(EC.element_to_be_clickable((By.XPATH, '//dialog' + confirm))).click()
    wait.until(EC.invisibility_of_element_located((By.CSS_SELECTOR, 'dialog[open]')))
    wait.until(EC.element_to_be_clickable((By.XPATH, '//button[contains(., "Enregistrer et définir les règles")]'))).click()
    wait.until(EC.element_to_be_clickable((By.XPATH, '//button[contains(., "Confirmer et analyser toutes les lignes")]'))).click()
    wait.until(EC.visibility_of_element_located((By.CSS_SELECTOR, '[data-testid="finding"]')))
    driver.save_screenshot(str(folder / 'native-review.png'))
    assert not driver.find_elements(By.CSS_SELECTOR, '[data-testid="global-error"]')
    session = call(driver, 'demo')
    source = Path(session['config']['source_path'])
    baseline = hashlib.sha256(source.read_bytes()).hexdigest()
    config = session['config']
    for mapping in config['mappings']:
        mapping['confirmed'] = True
    session = call(driver, 'configure', {'config': config}, session)
    session = call(driver, 'analyze', session=session)
    assert session['summary']['row_count'] == 3
    findings = call(driver, 'results', {'view': 'findings', 'limit': 100}, session)['items']
    phone = next(f for f in findings if f['kind'] == 'phone_multiple')
    chosen = phone['choices'][0]
    session = call(driver, 'decide', {'decisions': {phone['key']: {'action': 'set', 'value': chosen, 'allow_multiple': False, 'reason': 'Recette native'}}}, session)
    output = folder / 'resultat natif.xlsx'
    call(driver, 'export', {'path': str(output), 'allow_issues': True, 'overwrite': False}, session, fail=True)
    assert not output.exists()
    session = call(driver, 'undo', session=session)
    session = call(driver, 'redo', session=session)
    session = call(driver, 'analyze', session=session)
    result = call(driver, 'export', {'path': str(output), 'allow_issues': True, 'overwrite': False}, session)
    assert Path(result['path']).exists()
    workbook = load_workbook(output)
    assert str(workbook['Marches fictifs']['B2'].value) == chosen['value']
    assert workbook['Marches fictifs']['E2'].value == '000123'
    assert workbook['À vérifier'].max_row - 1 == session['summary']['findings']
    for sheet in workbook:
        for row in sheet:
            for cell in row:
                assert not cell.alignment.textRotation
    workbook.close()
    assert hashlib.sha256(source.read_bytes()).hexdigest() == baseline
    project = folder / 'projet.fdxproj'
    call(driver, 'project_save', {'path': str(project)}, session)
    restored = call(driver, 'project_open', {'path': str(project)})
    assert restored['dirty'] is True
    restored = call(driver, 'analyze', session=restored)
    assert restored['summary']['findings'] == session['summary']['findings']
    return {'ok': True, 'rows': 3, 'native_ui': True, 'native_bridge': True, 'source_unchanged': True, 'export_reopened': True, 'project_reopened': True}


if __name__ == '__main__':
    destination = Path('qa/native')
    destination.mkdir(parents=True, exist_ok=True)
    log = (destination / 'driver.log').open('w', encoding='utf-8')
    server = subprocess.Popen(['tauri-driver', '--native-driver', str(Path(sys.argv[2]).resolve())], stdout=log, stderr=subprocess.STDOUT)
    driver = None
    try:
        for attempt in range(30):
            try:
                driver = webdriver.Remote(command_executor='http://127.0.0.1:4444', options=TauriOptions())
                break
            except Exception:
                if server.poll() is not None or attempt == 29:
                    raise
                time.sleep(1)
        driver.set_script_timeout(120)
        report = check(driver, destination.resolve())
        (destination / 'verification.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
        print(json.dumps(report))
    finally:
        if driver:
            driver.quit()
        server.terminate()
        try:
            server.wait(timeout=10)
        except subprocess.TimeoutExpired:
            server.kill()
        log.close()
