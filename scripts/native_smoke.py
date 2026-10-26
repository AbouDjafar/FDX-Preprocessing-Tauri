"""Exercise the shipped Windows binary over its real Tauri bridge, never a mock."""
import hashlib
import json
import os
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid
from pathlib import Path

from openpyxl import load_workbook
from selenium import webdriver
from selenium.webdriver.common.by import By
from selenium.webdriver.edge.service import Service as EdgeService
from selenium.webdriver.remote.client_config import ClientConfig
from selenium.webdriver.support.ui import Select, WebDriverWait
from selenium.webdriver.support import expected_conditions as EC


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


def startup_diagnostics(folder):
    script = r'''
      $ErrorActionPreference = 'Continue'
      Get-CimInstance Win32_Process | Where-Object { $_.Name -match 'fdx|msedge|WebView' } |
        Select-Object Name,ProcessId,ParentProcessId,SessionId,CommandLine | ConvertTo-Json -Depth 3
      Get-Process | Where-Object { $_.ProcessName -match 'fdx|msedge|WebView' } |
        Select-Object Id,ProcessName,MainWindowTitle,MainWindowHandle,Responding | Format-List | Out-String
      try {
        Add-Type -AssemblyName System.Windows.Forms
        Add-Type -AssemblyName System.Drawing
        $bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
        $bitmap = New-Object System.Drawing.Bitmap $bounds.Width,$bounds.Height
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $graphics.CopyFromScreen($bounds.X,$bounds.Y,0,0,$bounds.Size)
        $bitmap.Save((Join-Path $env:FDX_NATIVE_DIAGNOSTICS_DIR 'native-startup-failure.png'))
        $graphics.Dispose()
        $bitmap.Dispose()
      } catch { Write-Output $_ }
    '''
    result = subprocess.run(
        ['powershell', '-NoProfile', '-Command', script],
        capture_output=True, text=True, timeout=20,
        env={**os.environ, 'FDX_NATIVE_DIAGNOSTICS_DIR': str(folder.resolve())},
    )
    (folder / 'startup-diagnostics.txt').write_text(result.stdout + result.stderr, encoding='utf-8')
    print(result.stdout[-10000:] + result.stderr[-3000:], flush=True)


def check(driver, folder, application_id):
    wait = WebDriverWait(driver, 30)
    wait.until(lambda d: 'FDX' in d.find_element(By.TAG_NAME, 'body').text)
    assert driver.execute_script('return getComputedStyle(document.body).backgroundColor') != 'rgb(255, 255, 255)'
    driver.set_window_size(1366, 768)
    wait.until(EC.element_to_be_clickable((By.CSS_SELECTOR, '[data-testid="try-demo"]')))
    print('Native interface ready', flush=True)
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
    print('Native analysis ready', flush=True)
    assert not driver.find_elements(By.CSS_SELECTOR, '[data-testid="global-error"]')
    finding = driver.find_elements(By.CSS_SELECTOR, '[data-testid="finding"]')[0]
    assert 'phone_multiple' in finding.text
    finding.find_element(By.XPATH, './/button[contains(., "Examiner et décider")]').click()
    decision = wait.until(EC.visibility_of_element_located((By.CSS_SELECTOR, 'dialog[open] select')))
    Select(decision).select_by_value('first')
    driver.find_element(By.XPATH, '//dialog//button[contains(., "Ajouter la décision au lot")]').click()
    wait.until(EC.element_to_be_clickable((By.XPATH, '//button[normalize-space()="Vérifier le lot"]'))).click()
    wait.until(EC.element_to_be_clickable((By.XPATH, '//button[normalize-space()="Analyser à nouveau"]')))
    driver.find_element(By.CSS_SELECTOR, '[data-testid="stage-export"]').click()
    wait.until(EC.visibility_of_element_located((By.XPATH, '//label[contains(., "J’autorise l’export avec les anomalies restantes")]/input'))).click()
    ui_output = folder / 'resultat interface.xlsx'
    # Drive the real Windows file picker without replacing the protected IPC bridge.
    with subprocess.Popen(
        ['powershell', '-NoProfile', '-File', str(Path(__file__).with_name('native_save_dialog.ps1').resolve()),
         '-ApplicationId', str(application_id), '-Destination', str(ui_output)],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
    ) as picker:
        driver.find_element(By.CSS_SELECTOR, '[data-testid="export-button"]').click()
        picker_log, _ = picker.communicate(timeout=40)
        (folder / 'file-picker.log').write_text(picker_log, encoding='utf-8')
        assert picker.returncode == 0, picker_log
    wait.until(EC.element_to_be_clickable((By.XPATH, '//dialog//button[normalize-space()="Exporter et vérifier"]'))).click()
    wait.until(lambda _: ui_output.exists())
    ui_book = load_workbook(ui_output)
    assert ui_book['Marches fictifs']['B2'].value == '699123456'
    assert not any(row[6] == 'phone_multiple' for row in ui_book['À vérifier'].iter_rows(min_row=2, values_only=True))
    ui_book.close()
    wait.until(EC.element_to_be_clickable((By.XPATH, '//button[normalize-space()="Ouvrir le classeur"]')))
    driver.save_screenshot(str(folder / 'native-export.png'))
    print('Native UI export reopened and verified', flush=True)
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
    print('Native project reopened and verified', flush=True)
    return {'ok': True, 'rows': 3, 'native_ui': True, 'native_ui_export': True, 'native_bridge': True, 'source_unchanged': True, 'export_reopened': True, 'project_reopened': True}


if __name__ == '__main__':
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        raise SystemExit('Native executable verification is restricted to GitHub Actions.')
    import winreg

    destination = Path('qa/native')
    destination.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment['TAURI_WEBVIEW_AUTOMATION'] = 'true'
    environment['TAURI_AUTOMATION'] = 'true'
    environment['WEBVIEW2_USER_DATA_FOLDER'] = tempfile.mkdtemp(prefix='fdx-native-webview-')
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        debug_port = listener.getsockname()[1]
    # Attach to a known loopback endpoint instead of relying on a shared
    # DevToolsActivePort file. Debugging is enabled only in this test process.
    environment['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS'] = (
        f'--remote-debugging-port={debug_port} --remote-debugging-address=127.0.0.1 --enable-automation'
    )
    service = EdgeService(
        executable_path=str(Path(sys.argv[2]).resolve()),
        service_args=['--verbose'], log_output=str(destination / 'driver.log'), env=environment,
    )
    options = webdriver.EdgeOptions()
    options.use_webview = True
    options.debugger_address = f'127.0.0.1:{debug_port}'
    driver = None
    service_started = False
    application = None
    policy = None
    previous_policy = None
    application_name = Path(sys.argv[1]).name
    application_log = (destination / 'application.log').open('w', encoding='utf-8')
    try:
        # Runtime 150+ ignores browser-argument environment overrides in elevated
        # hosts. Use the documented per-executable policy on this disposable CI
        # runner, restoring it below. The distributed application is unchanged.
        # https://github.com/MicrosoftEdge/WebView2Feedback/issues/5645
        policy = winreg.CreateKeyEx(
            winreg.HKEY_LOCAL_MACHINE,
            r'SOFTWARE\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments',
            0, winreg.KEY_SET_VALUE | winreg.KEY_QUERY_VALUE,
        )
        try:
            previous_policy = winreg.QueryValueEx(policy, application_name)
        except FileNotFoundError:
            pass
        winreg.SetValueEx(policy, application_name, 0, winreg.REG_SZ, environment['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS'])
        application = subprocess.Popen(
            [str(Path(sys.argv[1]).resolve())], env=environment,
            stdout=application_log, stderr=subprocess.STDOUT,
        )
        deadline = time.monotonic() + 45
        while True:
            if application.poll() is not None:
                raise RuntimeError(f'Native application exited before WebView2 was ready: {application.returncode}')
            try:
                with urllib.request.urlopen(f'http://127.0.0.1:{debug_port}/json/version', timeout=1) as response:
                    runtime = json.load(response)
                (destination / 'runtime.json').write_text(json.dumps(runtime, indent=2), encoding='utf-8')
                break
            except (OSError, urllib.error.URLError):
                if time.monotonic() >= deadline:
                    raise RuntimeError('Native WebView2 debugging endpoint did not become ready within 45 seconds')
                time.sleep(0.25)
        service.start()
        service_started = True
        print('Native WebView2 ready; attaching WebDriver to the running application', flush=True)
        driver = webdriver.Remote(
            command_executor=service.service_url, options=options,
            client_config=ClientConfig(service.service_url, timeout=60),
        )
        print('Native application session created', flush=True)
        driver.set_script_timeout(45)
        report = check(driver, destination.resolve(), application.pid)
        (destination / 'verification.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
        print(json.dumps(report))
    except BaseException:
        if driver:
            try:
                driver.save_screenshot(str(destination / 'native-failure.png'))
                (destination / 'failure.html').write_text(driver.page_source[:200000], encoding='utf-8')
            except Exception:
                pass
        else:
            try:
                startup_diagnostics(destination)
            except Exception as diagnostic_error:
                print(f'Startup diagnostics unavailable: {diagnostic_error}', flush=True)
        raise
    finally:
        if driver:
            try:
                driver.quit()
            except Exception:
                pass
        if service_started:
            service.stop()
        if application and application.poll() is None:
            subprocess.run(['taskkill', '/PID', str(application.pid), '/T', '/F'], capture_output=True, timeout=15)
        if policy is not None:
            if previous_policy is None:
                winreg.DeleteValue(policy, application_name)
            else:
                winreg.SetValueEx(policy, application_name, 0, previous_policy[1], previous_policy[0])
            winreg.CloseKey(policy)
        application_log.close()
