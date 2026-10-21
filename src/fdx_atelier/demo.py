"""Synthetic example generated locally; no user workbook is bundled."""

from pathlib import Path


def create_demo(folder):
    from openpyxl import Workbook
    from openpyxl.styles import Alignment

    folder = Path(folder)
    folder.mkdir(parents=True, exist_ok=True)
    source, dictionary = folder / "Exemple fictif.xlsx", folder / "Dictionnaire exemple.md"
    wb = Workbook()
    ws = wb.active
    ws.title = "Marchés fictifs"
    ws.append(["Entreprise", "Téléphone", "Montant", "Date de signature", "Identifiant"])
    for row in [
        ["  Atelier Horizon  ", "699123456 / 677123456", "1 234,50", "18 Fév. 2025", "000123"],
        ["Société Exemple", "699123456", "27 650,125", "31/09/2025", "000124"],
        ["Entreprise Démo", "+390498976845", 1250, "14/03/2025", "000125"],
    ]:
        ws.append(row)
    for cell in ws[1]:
        cell.alignment = Alignment(textRotation=90)
    ws.column_dimensions["A"].width = 28
    ws.freeze_panes = "A2"
    wb.save(source)
    wb.close()
    dictionary.write_text(
        "| Code | Libellé | Type | Taille | Obligatoire |\n"
        "|---|---|---|---|---|\n"
        "| NOM | Entreprise | Alphanumérique | 100 | Oui |\n"
        "| TEL | Téléphone | Numérique | | Non |\n"
        "| MON | Montant | Numérique | | Oui |\n"
        "| DATE | Date de signature | Date | JJ/MM/AAAA | Non |\n"
        "| ID | Identifiant | Alphanumérique | 10 | Non |\n",
        encoding="utf-8",
    )
    return source, dictionary
