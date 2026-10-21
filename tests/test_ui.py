from PySide6.QtCore import Qt
from PySide6.QtWidgets import QApplication, QFileDialog

from fdx_atelier.models import Cancelled, FieldRule
from fdx_atelier.storage import save_profile, save_project
from fdx_atelier.ui.app import STYLE, MainWindow


def test_full_ui_path_and_sorted_selection(qtbot, project, tmp_path, monkeypatch):
    QApplication.instance().setStyleSheet(STYLE)
    window = MainWindow()
    errors = []
    monkeypatch.setattr(window, "show_error", lambda error: errors.append(str(error)))
    qtbot.addWidget(window)
    window.show()
    window.config = project
    window.refresh_source()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.import_page.preview.columnCount() == 7
    window.navigate(1)
    assert window.dictionary_page.table.rowCount() == 6
    window.navigate(2)
    window.mapping_page.confirm()
    window.navigate(3)
    assert window.rules_page.field.count() == 6
    window.navigate(4)
    window.run_analysis()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.result is not None, errors
    assert window.result.row_count == 2
    window.review_page.table.sortByColumn(1, Qt.SortOrder.DescendingOrder)
    proxy = window.review_page.proxy
    for row in range(proxy.rowCount()):
        source_row = proxy.mapToSource(proxy.index(row, 0)).row()
        record = window.review_page.model.records[source_row]
        if record["kind"] == "phone_multiple":
            window.review_page.table.selectRow(row)
            break
    assert window.review_page.selected_records()[0]["row"] == 2
    window.review_page.action.setCurrentIndex(window.review_page.action.findData("first"))
    window.apply_selected()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert not any(f.kind == "phone_multiple" for f in window.result.findings)
    window.undo_decision()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert any(f.kind == "phone_multiple" for f in window.result.findings)
    window.redo_decision()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert not any(f.kind == "phone_multiple" for f in window.result.findings)
    window.navigate(5)
    target = tmp_path / "sortie.xlsx"
    window.export_page.path.setText(str(target))
    window.export_page.allow_issues.setChecked(True)
    window.run_export()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert target.exists(), errors
    assert not errors
    window.dirty = False
    window.close()


def test_reopen_session_and_load_profile_keep_saved_parameters(qtbot, project, tmp_path, monkeypatch):
    project.rules["TEL"] = FieldRule(phone_policy="clear")
    session = tmp_path / "reprise.fdxproj"
    profile = tmp_path / "regles.json"
    save_project(session, project, {})
    save_profile(profile, project)
    window = MainWindow()
    errors = []
    monkeypatch.setattr(window, "show_error", lambda error: errors.append(str(error)))
    qtbot.addWidget(window)
    window.config = project.model_copy(deep=True)
    window.refresh_source()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    window.navigate(3)
    window.dirty = False
    window.export_page.path.setText(str(tmp_path / "ancien.xlsx"))
    monkeypatch.setattr(QFileDialog, "getOpenFileName", lambda *args: (str(session), ""))
    window.open_session()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.step == 4
    assert window.config.rules["TEL"].phone_policy == "clear"
    assert "ancien.xlsx" not in window.export_page.path.text()
    window.run_analysis()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.result is not None, errors
    monkeypatch.setattr(QFileDialog, "getOpenFileName", lambda *args: (str(profile), ""))
    window.import_profile()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.step == 2
    assert not any(m.confirmed for m in window.config.mappings)
    assert window.result is None
    assert window.config.rules["TEL"].phone_policy == "clear"
    assert not errors
    window.dirty = False
    window.close()


def test_long_job_can_be_cancelled_without_partial_result(qtbot):
    window = MainWindow()
    qtbot.addWidget(window)
    outputs = []

    def work(cancel, progress):
        assert cancel.wait(10)
        raise Cancelled("Opération annulée")

    window.run_job(work, outputs.append)
    assert window.busy
    window.cancel_job()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert not outputs
    assert window.result is None
    assert window.sidebar.isEnabled()
    window.close()
