from pathlib import Path

from fdx_atelier.models import Decision, cell_key
from fdx_atelier.storage import load_profile, open_project, save_profile, save_project
from fdx_atelier.workbooks import sha256


def test_project_preserves_source_and_decisions(project, tmp_path):
    dictionary = tmp_path / "dictionnaire.md"
    dictionary.write_text("Code\tLibellé\tType\nNOM\tEntreprise\tTexte", encoding="utf-8")
    project.dictionary_path = str(dictionary)
    digest = sha256(project.source_path)
    decisions = {cell_key(project.sheet, 2, 2): Decision(action="clear")}
    path = tmp_path / "projet.fdxproj"
    save_project(path, project, decisions, [{"test": {"action": "keep"}}])
    loaded, actual, history = open_project(path, tmp_path / "cache")
    assert sha256(loaded.source_path) == digest
    assert loaded.original_path == project.source_path
    assert actual == decisions
    assert history[0]["test"]["action"] == "keep"
    assert Path(loaded.dictionary_path).read_text(encoding="utf-8") == dictionary.read_text(encoding="utf-8")


def test_profile_requires_mapping_confirmation(project, tmp_path):
    path = tmp_path / "profil.json"
    save_profile(path, project)
    assert project.source_path not in path.read_text(encoding="utf-8")
    loaded = load_profile(path, project)
    assert loaded.source_path == project.source_path
    assert not any(m.confirmed for m in loaded.mappings)
