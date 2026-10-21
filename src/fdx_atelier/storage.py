from __future__ import annotations

import json
import os
import sqlite3
import tempfile
from contextlib import closing
from pathlib import Path

from pydantic import TypeAdapter

from .models import Decision, ProjectConfig
from .workbooks import sha256


def save_profile(path: str | Path, config: ProjectConfig) -> None:
    path = Path(path)
    protected = {
        Path(v).resolve() for v in (config.source_path, config.original_path, config.dictionary_path) if v
    }
    if path.resolve() in protected or path.suffix.lower() != ".json":
        raise ValueError("Choisissez un fichier .json distinct des fichiers sources.")
    payload = config.model_dump(mode="json")
    for name in ("source_path", "original_path", "dictionary_path", "sheet"):
        payload[name] = ""
    _atomic_text(Path(path), json.dumps(payload, ensure_ascii=False, indent=2))


def load_profile(path: str | Path, current: ProjectConfig) -> ProjectConfig:
    loaded = ProjectConfig.model_validate_json(Path(path).read_text(encoding="utf-8"))
    for name in ("source_path", "original_path", "dictionary_path", "sheet"):
        setattr(loaded, name, getattr(current, name))
    # Imported mappings are proposals until checked against this workbook.
    for mapping in loaded.mappings:
        mapping.confirmed = False
    return loaded


def _atomic_text(path: Path, text: str) -> None:
    with tempfile.NamedTemporaryFile(
        mode="w", encoding="utf-8", dir=path.parent, suffix=".tmp", delete=False
    ) as stream:
        temp = Path(stream.name)
        stream.write(text)
    try:
        os.replace(temp, path)
    finally:
        temp.unlink(missing_ok=True)


def save_project(
    path: str | Path,
    config: ProjectConfig,
    decisions: dict[str, Decision],
    history: list[dict] | None = None,
    result=None,
    ui_state: dict | None = None,
) -> None:
    path = Path(path)
    if path.resolve() in {
        Path(config.source_path).resolve(),
        Path(config.original_path or config.source_path).resolve(),
    }:
        raise ValueError("Le projet ne peut pas remplacer le fichier source.")
    with tempfile.NamedTemporaryFile(dir=path.parent, suffix=".db", delete=False) as stream:
        temp = Path(stream.name)
    try:
        with closing(sqlite3.connect(temp)) as db, db:
            db.execute("CREATE TABLE project (key TEXT PRIMARY KEY, value BLOB NOT NULL)")
            items = {
                "version": "1",
                "config": config.model_dump_json(),
                "decisions": TypeAdapter(dict[str, Decision]).dump_json(decisions),
                "history": json.dumps(history or [], ensure_ascii=False, default=str),
                "source": Path(config.source_path).read_bytes(),
                "source_hash": sha256(config.source_path),
                "dictionary": Path(config.dictionary_path).read_bytes() if config.dictionary_path else b"",
                "result": result.model_dump_json() if result is not None else "",
                "ui_state": json.dumps(ui_state or {}, ensure_ascii=False),
            }
            db.executemany("INSERT INTO project(key, value) VALUES (?, ?)", items.items())
        os.replace(temp, path)
    finally:
        temp.unlink(missing_ok=True)


def open_project(
    path: str | Path, cache_directory: str | Path
) -> tuple[ProjectConfig, dict[str, Decision], list]:
    uri = Path(path).resolve().as_uri() + "?mode=ro"
    with closing(sqlite3.connect(uri, uri=True)) as db:
        data = dict(db.execute("SELECT key, value FROM project"))
    if data.get("version") != "1":
        raise ValueError("Version du projet non prise en charge.")
    import hashlib

    if hashlib.sha256(data["source"]).hexdigest() != data["source_hash"]:
        raise ValueError("La copie source enregistrée dans le projet est endommagée.")
    config = ProjectConfig.model_validate_json(data["config"])
    cache = Path(cache_directory)
    cache.mkdir(parents=True, exist_ok=True)
    folder = Path(tempfile.mkdtemp(prefix="session-", dir=cache))
    source = folder / "source.xlsx"
    source.write_bytes(data["source"])
    config.original_path = config.original_path or config.source_path
    config.source_path = str(source)
    if data["dictionary"]:
        dictionary = folder / "dictionnaire.txt"
        dictionary.write_bytes(data["dictionary"])
        config.dictionary_path = str(dictionary)
    decisions = TypeAdapter(dict[str, Decision]).validate_json(data["decisions"])
    return config, decisions, json.loads(data.get("history", "[]"))


def project_ui_state(path: str | Path) -> dict:
    uri = Path(path).resolve().as_uri() + "?mode=ro"
    with closing(sqlite3.connect(uri, uri=True)) as db:
        row = db.execute("SELECT value FROM project WHERE key = 'ui_state'").fetchone()
    state = json.loads(row[0]) if row else {}
    return state if isinstance(state, dict) else {}
