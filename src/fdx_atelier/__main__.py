from __future__ import annotations

import argparse
import json
import multiprocessing
import sys
from pathlib import Path

from . import __version__


def main(argv=None):
    parser = argparse.ArgumentParser(prog="fdx-atelier", description="FDX Atelier Données")
    parser.add_argument("--version", action="version", version=__version__)
    commands = parser.add_subparsers(dest="command")
    commands.add_parser("gui", help="Ouvrir l'interface Windows")
    inspect = commands.add_parser("inspect", help="Examiner un classeur sans le modifier")
    inspect.add_argument("workbook")
    inspect.add_argument("--header-row", type=int, default=1)
    selftest = commands.add_parser("selftest", help="Vérifier le moteur et les quatre étapes hors ligne")
    selftest.add_argument("--output", required=True)
    selftest.add_argument("--scale", type=float, default=1.0)
    for name in ("analyze", "export"):
        command = commands.add_parser(name)
        command.add_argument("--config", required=True, help="Configuration JSON complète")
        command.add_argument("--decisions", help="Décisions JSON facultatives")
        command.add_argument("--output", required=True)
        if name == "export":
            command.add_argument("--allow-issues", action="store_true")
            command.add_argument("--overwrite", action="store_true")
    args = parser.parse_args(argv)
    if args.command in (None, "gui"):
        from .ui.app import launch

        return launch()
    try:
        if args.command == "selftest":
            from .selftest import run

            run(args.output, args.scale)
            return 0
        if args.command == "inspect":
            from .workbooks import inspect_workbook

            print(
                json.dumps(inspect_workbook(args.workbook, args.header_row), ensure_ascii=False, default=str)
            )
            return 0
        from pydantic import TypeAdapter

        from .engine import analyze
        from .exporter import export_excel
        from .models import Decision, ProjectConfig

        config = ProjectConfig.model_validate_json(Path(args.config).read_text(encoding="utf-8"))
        decisions = (
            TypeAdapter(dict[str, Decision]).validate_json(Path(args.decisions).read_text(encoding="utf-8"))
            if args.decisions
            else {}
        )
        result = analyze(config, decisions)
        if args.command == "analyze":
            output = Path(args.output).resolve()
            if output in {
                Path(config.source_path).resolve(),
                Path(config.original_path or config.source_path).resolve(),
            }:
                raise ValueError("Le rapport ne peut pas remplacer le fichier source.")
            output.write_text(result.model_dump_json(indent=2), encoding="utf-8")
        else:
            export_excel(config, result, args.output, args.allow_issues, args.overwrite)
        print(
            json.dumps(
                {"rows": result.row_count, "changes": len(result.changes), "issues": len(result.findings)},
                ensure_ascii=False,
            )
        )
        return 0
    except Exception as exc:
        print(f"Erreur : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    multiprocessing.freeze_support()
    raise SystemExit(main())
