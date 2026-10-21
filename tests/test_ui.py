import pytest
from openpyxl import load_workbook
from PySide6.QtCore import Qt
from PySide6.QtWidgets import QApplication, QFileDialog, QPlainTextEdit

from fdx_atelier.engine import analyze
from fdx_atelier.models import Cancelled, Decision, FieldRule
from fdx_atelier.storage import project_ui_state, save_profile, save_project
from fdx_atelier.ui.app import MainWindow
from fdx_atelier.ui.session import SessionController, decision_stamp
from fdx_atelier.ui.theme import apply_theme


@pytest.fixture
def window(qtbot, monkeypatch):
    apply_theme(QApplication.instance())
    widget = MainWindow()
    widget.errors = []
    monkeypatch.setattr(widget, "show_error", lambda error: widget.errors.append(str(error)))
    monkeypatch.setattr(widget, "may_discard", lambda: True)
    qtbot.addWidget(widget)
    widget.show()
    yield widget
    if widget.busy:
        widget.cancel_job()
        qtbot.waitUntil(lambda: not widget.busy, timeout=30000)
    widget.dirty = False
    widget.close()


def ready(window, project, qtbot):
    window.config = project
    window.refresh_source()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    window.navigate(1)
    window.run_analysis(lambda: window.navigate(2))
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.result is not None, window.errors


def choose_problem(window, kind):
    window.review_page.tabs.setCurrentIndex(0)
    window.review_page.category.setCurrentIndex(window.review_page.category.findData(kind))
    assert window.review_page.current_record()["kind"] == kind


def test_guided_demo_entire_path(window, qtbot, tmp_path):
    assert not window.next_button.isEnabled()
    window.load_demo()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.import_page.preview.rowCount() == 3
    window.next_action()
    assert window.step == 1
    assert not window.prepare_page.tabs.isTabVisible(2)
    window.next_action()
    assert all(m.confirmed for m in window.config.mappings if m.field_code)
    window.next_action()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.step == 2
    assert window.result.row_count == 3
    choose_problem(window, "phone_multiple")
    window.review_page.action.setCurrentIndex(window.review_page.action.findData("first"))
    window.apply_selected()
    assert not window.busy
    assert not window.session.current
    assert window.review_page.current_record()["pending"]
    assert "Vérifier mes corrections" == window.next_button.text()
    window.next_action()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert not any(f.kind == "phone_multiple" for f in window.result.findings)
    choose_problem(window, "date_invalid")
    window.review_page.action.setCurrentIndex(window.review_page.action.findData("proposal"))
    window.apply_selected()
    window.next_action()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert not window.result.findings
    window.next_action()
    assert window.step == 3
    target = tmp_path / "résultat.xlsx"
    window.export_page.path.setText(str(target))
    window.next_action()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert target.exists(), window.errors
    wb = load_workbook(target)
    assert wb.active["B2"].value == "699123456"
    assert wb.active["C2"].value == 1234.5
    assert wb.active["E2"].value == "000123"
    assert wb.active["A1"].alignment.textRotation == 0
    wb.close()
    assert window.export_page.open_file.isVisible()
    assert window.next_button.text() == "Ouvrir mon fichier"
    assert not window.errors


def test_decisions_are_batched_and_undoable(window, project, qtbot):
    ready(window, project, qtbot)
    choose_problem(window, "phone_multiple")
    result = window.result
    window.review_page.action.setCurrentIndex(window.review_page.action.findData("first"))
    window.apply_selected()
    assert window.result is result
    assert not window.busy
    assert not window.session.current
    assert len(window.history) == 1
    window.undo_decision()
    assert not window.decisions
    assert window.session.current
    window.redo_decision()
    assert window.decisions
    assert not window.session.current
    window.run_analysis()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.session.current
    assert not any(f.kind == "phone_multiple" for f in window.result.findings)


def test_sorted_advanced_selection_keeps_coordinates(window, project, qtbot):
    ready(window, project, qtbot)
    window.advanced_toggle.setChecked(True)
    window.review_page.table.sortByColumn(1, Qt.SortOrder.DescendingOrder)
    choose_problem(window, "phone_multiple")
    window.review_page.table.selectRow(0)
    record = window.review_page.selected_records()[0]
    assert record["row"] == 2 and record["column"] == 2
    window.review_page.action.setCurrentIndex(window.review_page.action.findData("first"))
    window.apply_selected()
    assert next(iter(window.decisions)).endswith("!2:2")
    window.run_analysis()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert not any(f.kind == "phone_multiple" for f in window.result.findings)
    assert not window.errors


def test_actions_adapt_to_problem(window, project, qtbot):
    ready(window, project, qtbot)
    choose_problem(window, "date_invalid")
    actions = [window.review_page.action.itemData(i) for i in range(window.review_page.action.count())]
    assert "first" not in actions
    assert "choose" in actions
    choose_problem(window, "phone_multiple")
    assert window.review_page.action.findData("all") >= 0


def test_stale_export_blocked(window, project, qtbot, tmp_path):
    ready(window, project, qtbot)
    choose_problem(window, "phone_multiple")
    window.review_page.action.setCurrentIndex(window.review_page.action.findData("clear"))
    window.apply_selected()
    target = tmp_path / "interdit.xlsx"
    window.export_page.path.setText(str(target))
    window.run_export()
    assert not target.exists()
    assert "Vérifiez" in window.errors[-1]


def test_back_navigation_preserves_rules(window, project, qtbot):
    ready(window, project, qtbot)
    window.navigate(1)
    window.prepare_page.tabs.setCurrentIndex(1)
    rules = window.rules_page
    rules.field.setCurrentIndex(rules.field.findData("TEL"))
    rules.phone_policy.setCurrentIndex(rules.phone_policy.findData("first"))
    window.navigate(0)
    assert window.config.rules["TEL"].phone_policy == "first"
    window.navigate(1)
    window.prepare_page.tabs.setCurrentIndex(1)
    rules.field.setCurrentIndex(rules.field.findData("TEL"))
    assert rules.phone_policy.currentData() == "first"


def test_replacements_and_allowed_values_without_json(window, project, qtbot):
    ready(window, project, qtbot)
    window.navigate(1)
    window.advanced_toggle.setChecked(True)
    window.prepare_page.tabs.setCurrentIndex(2)
    assert isinstance(window.dictionary_page.table.cellWidget(0, 8), QPlainTextEdit)
    window.dictionary_page.table.cellWidget(0, 8).setPlainText("Entreprise A\nEntreprise B")
    window.prepare_page.tabs.setCurrentIndex(1)
    window.rules_page.add_replacement("Entreprise A", "A")
    window.prepare_page.collect()
    assert window.config.fields[0].allowed_values == ["Entreprise A", "Entreprise B"]
    assert window.config.rules["NOM"].replacements["Entreprise A"] == "A"
    assert not window.errors


def test_legacy_session_and_profile(window, project, tmp_path, qtbot, monkeypatch):
    project.rules["TEL"] = FieldRule(phone_policy="clear")
    session = tmp_path / "ancien.fdxproj"
    profile = tmp_path / "profil.json"
    save_project(session, project, {})
    save_profile(profile, project)
    assert project_ui_state(session) == {}
    monkeypatch.setattr(QFileDialog, "getOpenFileName", lambda *args: (str(session), ""))
    window.open_session()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.step == 2
    assert window.config.rules["TEL"].phone_policy == "clear"
    window.dirty = False
    monkeypatch.setattr(QFileDialog, "getOpenFileName", lambda *args: (str(profile), ""))
    window.import_profile()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.step == 1
    assert not any(m.confirmed for m in window.config.mappings)
    assert window.result is None
    assert not window.errors


def test_ui_state_saved_and_reloaded(window, project, tmp_path, qtbot, monkeypatch):
    ready(window, project, qtbot)
    window.advanced_toggle.setChecked(True)
    path = tmp_path / "projet.fdxproj"
    monkeypatch.setattr(QFileDialog, "getSaveFileName", lambda *args: (str(path), ""))
    assert window.save_session()
    assert project_ui_state(path)["advanced"]
    monkeypatch.setattr(QFileDialog, "getOpenFileName", lambda *args: (str(path), ""))
    window.open_session()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.advanced
    assert not window.errors


def test_long_job_cancelled(window, qtbot):
    outputs = []

    def work(cancel, progress):
        assert cancel.wait(10)
        raise Cancelled("Opération annulée")

    window.run_job(work, outputs.append)
    window.cancel_job()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert not outputs
    assert window.result is None
    assert not window.errors
    assert window.nav_buttons[0].isEnabled()


def test_source_change_clears_stale_preview(window, project, qtbot):
    ready(window, project, qtbot)
    window.set_source(project.source_path)
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.result is None
    assert window.decisions == {}
    assert window.config.mappings == []


def test_controller_rejects_outdated_worker_result(project):
    state = SessionController(config=project)
    result = analyze(project)
    stamp = decision_stamp({})
    state.queue({"test": Decision(action="clear")}, "Vider")
    assert not state.accept(result, stamp)
    state.undo()
    assert state.accept(result, stamp)
    assert state.current


def test_no_technical_input_in_guided_mode(window, project, qtbot):
    ready(window, project, qtbot)
    window.navigate(1)
    assert window.mapping_page.table.isColumnHidden(0)
    assert not window.mapping_page.table.cellWidget(0, 0).isVisible()
    assert not window.mapping_page.recalculate.isVisible()
    window.prepare_page.tabs.setCurrentIndex(1)
    assert not window.rules_page.replacements.isVisible()
    assert not window.rules_page.seg_boxes["source"].isVisible()


def test_new_export_options_invalidate_published_path(window, project, qtbot):
    ready(window, project, qtbot)
    window.navigate(3)
    window.export_page.published_path = "resultat.xlsx"
    window.export_page.harmonized.setChecked(False)
    assert window.export_page.published_path is None
    assert window.next_button.text() == "Créer mon fichier Excel"


def test_batch_actions_use_common_choices(window, project, qtbot):
    ready(window, project, qtbot)
    window.advanced_toggle.setChecked(True)
    from PySide6.QtCore import QItemSelectionModel

    selection = window.review_page.table.selectionModel()
    selection.select(
        window.review_page.proxy.index(0, 0),
        QItemSelectionModel.SelectionFlag.Select | QItemSelectionModel.SelectionFlag.Rows,
    )
    selection.select(
        window.review_page.proxy.index(1, 0),
        QItemSelectionModel.SelectionFlag.Select | QItemSelectionModel.SelectionFlag.Rows,
    )
    assert len(window.review_page.selected_records()) == 2
    assert window.review_page.action.findData("first") == -1
    assert window.review_page.action.findData("choose") == -1


def test_empty_search_and_category_reset(window, project, qtbot):
    ready(window, project, qtbot)
    choose_problem(window, "phone_multiple")
    window.review_page.action.setCurrentIndex(window.review_page.action.findData("first"))
    window.apply_selected()
    window.run_analysis()
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.review_page.category.currentData() == ""
    assert window.review_page.current_record()["kind"] == "date_invalid"
    window.review_page.search.setText("aucun resultat")
    assert window.review_page.current_record() is None
    assert not window.review_page.apply.isEnabled()


def test_unrecognized_dictionary_can_be_completed(window, project, qtbot, tmp_path):
    note = tmp_path / "regles.txt"
    note.write_text("Conserver les informations non reconnues.", encoding="utf-8")
    window.set_dictionary(note)
    window.set_source(project.source_path)
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert window.next_button.isEnabled()
    window.next_action()
    assert window.step == 1
    assert window.advanced
    assert window.prepare_page.tabs.currentIndex() == 2
    window.dictionary_page.add()
    window.dictionary_page.collect()
    assert len(window.config.fields) == 1
    assert not window.errors


def test_header_past_last_row_blocks_continue(window, project, qtbot):
    ready(window, project, qtbot)
    window.navigate(0)
    window.import_page.header.setValue(20)
    qtbot.waitUntil(lambda: not window.busy, timeout=30000)
    assert not window.next_button.isEnabled()
    assert window.import_page.preview.rowCount() == 0
