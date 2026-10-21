# FDX Atelier Données 1.1

Application Windows française de préparation de fichiers Excel, avec dictionnaire de données Markdown ou texte. Tous les traitements restent sur le poste ; aucune IA externe, aucun transfert des classeurs.

## Utiliser l'application

Télécharger l'installateur ou la version portable depuis les [releases](https://github.com/AbouDjafar/FDX-Preprocessing/releases). Python et Excel ne sont pas nécessaires pour utiliser l'exécutable. La version portable doit rester avec son dossier `_internal`.

1. **Mes fichiers** : choisir le classeur `.xlsx`, son dictionnaire `.md` ou `.txt`, puis confirmer la feuille et la ligne des noms de colonnes.
2. **Préparer** : confirmer les correspondances et formats proposés. Les paramètres particuliers restent dans l'onglet Traitements ; le mode avancé donne accès au dictionnaire complet.
3. **Vérifier** : parcourir les problèmes, préparer les décisions, puis vérifier le lot. Les conversions simples sont automatiques et peuvent être annulées depuis les modifications préparées.
4. **Enregistrer** : choisir le nom et l'emplacement du résultat. Le classeur est rouvert et vérifié avant publication.

Le bouton **Essayer un exemple** crée un petit fichier fictif pour découvrir le parcours. Le menu Projet permet de reprendre une session ou de réutiliser un profil. Le [guide utilisateur](docs/GUIDE.md) détaille les décisions et les limites.

## Garanties et limites

- Sources intactes ; analyse de toutes les lignes de la feuille choisie ; aucune suppression automatique de ligne.
- Texte horizontal et nombres sans groupement des milliers. Les décimales et identifiants textuels restent conservés.
- Téléphones multiples, dates impossibles, troncatures et cellules à vider nécessitent une règle ou une décision explicite.
- Rapports facultatifs : `Contrôle`, `Nom_original`, `À vérifier`. Les problèmes résolus ne figurent plus dans le rapport reconstruit.
- `.xlsx` uniquement, une feuille traitée à la fois, maximum 5 millions de cellules. Les dessins, graphiques, tableaux croisés, macros et objets avancés sont refusés pour éviter une perte de contenu.
- Les projets `.fdxproj` contiennent une copie des données : ils doivent rester dans un emplacement approprié.
- Exécutable non signé : Windows peut afficher un avertissement de réputation.

## Développer et tester

Windows x64, Python 3.12. Les versions validées sont dans `requirements-lock.txt`.

```powershell
python -m venv .venv
.\.venv\Scripts\python.exe -m pip install -r requirements-lock.txt
.\.venv\Scripts\python.exe -m pip install -e . --no-build-isolation
.\.venv\Scripts\python.exe -m fdx_atelier
.\.venv\Scripts\python.exe -m pytest -q
.\.venv\Scripts\python.exe -m ruff check src tests scripts
```

Le moteur reste indépendant de Qt : `analyze(config, decisions)` et `export_excel(config, result, destination)` sont compatibles avec la version 1.0. Les commandes `inspect`, `analyze` et `export` restent disponibles avec `--help`. Les anciens profils et projets sont lisibles.

```powershell
.\.venv\Scripts\python.exe -m fdx_atelier selftest --output qa\verification.json
.\.venv\Scripts\python.exe -m fdx_atelier selftest --output qa\scale200\verification.json --scale 2
.\scripts\build_windows.ps1 -ISCC 'C:\chemin\ISCC.exe'
.\scripts\verify_windows.ps1 -TestInstaller
```

La construction utilise PyInstaller et Inno Setup. La configuration alternative Nuitka est conservée mais sa compilation complète n'est pas validée. La CI Windows teste le moteur, l'interface, l'exécutable et une installation temporaire.

Les tests créent des données fictives. La recette sur données personnelles est uniquement locale, via `scripts/acceptance.py --root 'dossier FDX' --large` ; son dossier `qa/` n'est jamais publié. Les icônes Lucide et les notices des composants tiers accompagnent la distribution.
