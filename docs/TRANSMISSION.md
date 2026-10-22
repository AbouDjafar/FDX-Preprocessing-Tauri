# Transmission du projet FDX

Mémo de reprise humain/IA. Référence : version 1.1.0. Périmètre : métier, traitement, architecture et livraison ; aucun choix d'apparence. Le code et les tests priment si ce document devient obsolète.

## 1. L'essentiel

- **But** : fiabiliser des classeurs de suivi des marchés publics avant reporting ou analyse, pas gérer juridiquement une procédure de marché.
- **Origine** : harmoniser trois familles de données, passation, marchés signés et exécution, avec leurs dictionnaires et des sorties comparables.
- **Autorité** : pièces sources et dictionnaire validé par le métier. Une intuition, une similarité de libellé ou une date plausible n'est pas une preuve.
- **Principe** : proposer les cas ambigus, automatiser seulement les conversions autorisées, conserver une trace et ne jamais remplacer la source.
- **Cible** : application Windows française, locale, sans IA externe ; moteur utilisable sans interface. Utilisateur non spécialiste, paramètres avancés conservés.
- **Livraison** : sources sur `main`, compilation sur GitHub Actions, binaires dans les releases, téléchargement manuel par l'utilisateur.

## 2. Procédure métier

1. **Identifier les données** : importer `.xlsx` et dictionnaire `.md`/`.txt` ; proposer puis faire confirmer feuille et ligne d'en-tête. Ne jamais imposer un nom de feuille ou confondre aperçu et analyse complète.
2. **Valider le dictionnaire** : code unique, libellé, type, rôle, longueur, caractère obligatoire, format de date, valeurs autorisées, bornes et motif éventuel. Le texte libre non reconnu reste à compléter manuellement ; il n'est pas exécuté.
3. **Associer les colonnes** : rapprochement par code, libellé normalisé et similarité. Les positions Excel identifient les colonnes, même avec des en-têtes identiques. Confirmation obligatoire ; un champ ne cible pas deux colonnes. Ignorer une colonne conserve ses valeurs.
4. **Choisir les traitements** : séparateurs numériques, ordre des dates, pays des téléphones, remplacements exacts, segmentation et relations chronologiques. Les profils importés doivent être reconfirmés pour chaque classeur.
5. **Analyser toutes les lignes** après l'en-tête : préparer les conversions et relever les anomalies, y compris celles cumulées dans une cellule. Ne pas limiter le traitement à l'aperçu ou aux anciennes lignes signalées.
6. **Décider** : correction individuelle ou groupée, avec origine et proposition. Accumuler les décisions sans relire le classeur à chaque clic ; recalculer lors de la vérification du lot. Une décision individuelle prime sur la règle générale.
7. **Exporter** : résultat à jour, nom choisi, validation explicite des anomalies conservées et des remplacements éventuels. Écrire, rouvrir, contrôler, puis publier le fichier final.

## 3. Règles de traitement

| Sujet | Comportement à préserver |
|---|---|
| Espaces et texte | Normaliser selon les paramètres ; remplacements exacts configurés, pas de réécriture libre du contenu métier. |
| Nombres | Interpréter séparateurs décimal/de groupement et unités autorisées (`FCFA`, `XAF` par défaut). Utiliser `Decimal` pour l'analyse ; aucun arrondi métier implicite. Un entier ne doit pas absorber une fraction. |
| Identifiants | Stockage texte pour NIU, références et assimilés. Ne pas tronquer pour faire entrer une valeur dans une longueur. Des zéros déjà perdus dans la source numérique ne sont pas récupérables par déduction. |
| Téléphones | Stockage texte attendu ; région initiale `CM`, modifiable. `phonenumbers` vérifie la plausibilité, pas l'appartenance à l'entreprise. Ne jamais raccourcir un numéro international simplement parce qu'il est long. |
| Téléphones multiples | Par défaut : signaler. Choix explicites : premier, premier valide, numéro choisi, tous avec séparateur ou cellule vide. « Un seul numéro » était une consigne de traitement particulière, pas une règle universelle. |
| Dates | Accepter les interprétations compatibles avec l'ordre confirmé. Date impossible ou réparation proposée : confirmation requise ; ne pas inventer un jour, mois ou exercice. |
| Chronologie | Relations entre deux champs configurées et désactivables. Comparer des dates reconnues ; ne pas changer une année parce qu'elle précède ou suit une signature. Les anciennes exclusions d'exercices étaient contextuelles. |
| Contraintes | Obligatoire, longueur, motif, valeurs autorisées et bornes. Conserver/signaler par défaut ; vider ou tronquer uniquement sur règle ou décision explicite. |
| Segmentation | Extraire entreprise, BP (boîte postale), téléphone et localisation vers des colonnes existantes choisies. Destinations distinctes ; pas d'écrasement d'une valeur différente sans autorisation. |
| Formules | Les conserver et les signaler ; ne pas les remplacer par une correction métier. Excel recalculera les résultats à l'ouverture. |

Vider une cellule n'est jamais supprimer une ligne. « Conserver et signaler » ne transforme pas une anomalie en donnée conforme. Une correction peut révéler une autre anomalie lors du recalcul.

## 4. Sorties et invariants

- Conserver feuilles, dimensions, coordonnées, fusions, filtres et volets de la source ; ne pas reconstruire le tableau dans un nouveau modèle.
- Modifier uniquement les valeurs sélectionnées/destinataires autorisés. Exception globale voulue : rotation du texte à zéro et formats numériques sans groupement, dans toutes les feuilles conservées ou produites. Cela ne signifie pas aligner tous les nombres à gauche.
- Préserver valeurs numériques, décimales représentables, dates et identifiants textuels. Les limites de précision d'Excel subsistent ; une conservation non vérifiée ne doit pas être déclarée réussie.
- Rapports facultatifs : **Contrôle** = dictionnaire des champs associés et indicateurs ; **Nom_original** = valeurs sources des cellules modifiées et sources segmentées ; **À vérifier** = anomalies restantes, reconstruites depuis le résultat final. Cette dernière compte des anomalies, pas nécessairement des cellules ou lignes distinctes.
- Collision de rapports : suffixer par défaut ; remplacement explicite seulement. Mode sans rapports = structure source conservée avec valeurs et formats autorisés corrigés.
- Source protégée par SHA-256, contrôlée pendant analyse/export ; paramètres identifiés par empreinte. Résultat périmé ou source modifiée = nouvel examen obligatoire.
- Export : temporaire dans le dossier cible, réouverture et comparaison, contrôle des formats/compteurs, puis `os.replace`. Nettoyer le temporaire en cas d'échec ; ne pas annoncer de succès sur simple sauvegarde.

## 5. Carte technique

Base : `src/fdx_atelier/`. Les chemins ci-dessous sont relatifs au dépôt, jamais à un poste particulier.

| Fichier | Responsabilité / point de reprise |
|---|---|
| `models.py` | Pydantic : `FieldSpec`, `FieldRule`, `ColumnMapping`, `ProjectConfig`, `Decision`, `Finding`, `CellChange`, `AnalysisResult`. |
| `dictionary.py` | Lecture du dictionnaire, inférence de champs, notes non reconnues, propositions RapidFuzz. |
| `transforms.py` | Nettoyage, nombres, dates, téléphones et segmentation ; commencer ici pour une règle de conversion. |
| `engine.py` | `analyze` : parcours intégral, transformations, décisions prioritaires, validations par cellule, contraintes Pandera, chronologie, journal des changements. |
| `workbooks.py` | Précontrôle, chargement préservant le contenu pris en charge, aperçu, suggestions d'en-tête/feuille, empreintes et annulation. |
| `exporter.py` | `export_excel`, formats sans groupement, rapports, écriture temporaire et vérification de conservation. |
| `storage.py` | Profils JSON et projets SQLite ; écritures atomiques, copies sources et contrôle d'intégrité. |
| `ui/session.py` | `SessionController` indépendant de Qt et des entrées/sorties : état, décisions, annulation/rétablissement, résultat courant. |
| `ui/app.py`, `ui/prepare.py`, `ui/pages.py`, `ui/review.py`, `ui/tables.py` | Orchestration du parcours, collecte des paramètres, tâches de fond, sélection/filtrage et décisions. Ne pas y dupliquer les règles métier. |
| `__main__.py`, `selftest.py`, `demo.py` | Commandes, vérification autonome et génération d'exemples fictifs. |

Dépendances structurantes : Python 3.12, openpyxl (classeur), Pydantic (contrats), pandas/Pandera (contraintes tabulaires), RapidFuzz (rapprochements), phonenumbers (téléphones), PySide6 (application native). Versions de référence : `requirements-lock.txt` ; déclarations : `pyproject.toml`.

## 6. Contrats à ne pas casser

- API moteur : `analyze(config, decisions=None, cancel=None, progress=None)` retourne `AnalysisResult`. Export : `export_excel(config, result, destination, allow_issues=False, overwrite=False, cancel=None, progress=None)` retourne le chemin final.
- CLI conservée : `inspect`, `analyze`, `export`, `selftest`, lancement de l'application. Consulter `--help` pour les arguments exacts.
- Clé de cellule : `cell_key(feuille, ligne, colonne)` produit `feuille!ligne:colonne` ; indices Excel à partir de 1. Un tri d'affichage ne doit jamais déplacer la décision vers une autre cellule.
- Décisions moteur : `keep`, `set`, `clear`. Les actions métier sont traduites en ces décisions ; `allow_multiple` permet une conservation explicite de plusieurs numéros.
- Résultat courant : empreinte de configuration **et** empreinte des décisions identiques à celles analysées. `SessionController.accept` rejette un résultat devenu obsolète. Les choix purement liés aux rapports sont exclus de l'empreinte d'analyse.
- Profils/projets : schéma version `1` malgré la version applicative `1.1.0`. Profils sans chemins sources, associations retransformées en propositions à l'import. Projet `.fdxproj` = SQLite avec source, dictionnaire, paramètres, décisions et historique ; données confidentielles, non chiffrées par ce mécanisme.
- `open_project` garde son retour `(config, decisions, history)`. Métadonnées additionnelles via `project_ui_state`, facultatives pour lire les anciens projets. Pas de migration silencieuse destructive.
- Traitements longs : progression, annulation coopérative (`Cancelled`) et tâches de fond. Une nouvelle vérification applique les décisions accumulées depuis la source.

## 7. Limites et améliorations possibles

- `.xlsx`, un fichier et une feuille traitée à la fois ; plafond actuel de cinq millions de cellules dans cette feuille. Macros, `.xls`, chiffrement et plusieurs objets Excel avancés hors périmètre ; conserver le refus préventif de `preflight` tant que la conservation n'est pas démontrée.
- Le parseur de dictionnaire est heuristique, pas une compréhension générale du texte. Les rapprochements restent des propositions. Ni lois de la commande publique ni règles propres à une organisation ne doivent être supposées universelles.
- Dates : années à deux chiffres actuellement interprétées dans les années 2000 ; `205`/`225` peuvent produire une proposition `2025` à confirmer. Rendre ces conventions historiques configurables et testées plutôt que les étendre silencieusement.
- Segmentation : marqueurs tels que BP et téléphone, pas de reconnaissance exhaustive d'adresses. La séparation de blocs téléphoniques concaténés repose aussi sur des hypothèses locales. Étendre par cas synthétiques et validation humaine.
- Excel n'offre pas une précision décimale arbitraire. L'export convertit `Decimal` en nombre Excel puis vérifie ; garder les identifiants longs en texte.
- Priorités proposées, non promises comme livrées : configuration des heuristiques de dates, meilleure segmentation, import de dictionnaires plus robuste, performance des décisions groupées massives, réduction de la mémoire. Ajouter lots/macros seulement après définition des garanties.

## 8. Vérifier et livrer

- Base de référence : **54 tests passaient** après la version 1.1.0 ; constat historique, pas résultat de chaque intervention. Vérifications locales ciblées : `python -m pytest -q` et `python -m ruff check src tests scripts` dans un environnement de développement préparé.
- Tests à cibler : `test_dictionary_transforms.py` (conversions), `test_engine_export.py` et `test_regressions.py` (règles/conservation), `test_storage.py` (reprise), `test_ui.py` (parcours/décisions), `test_release.py` (publication).
- Recette minimale : dates ambiguës/impossibles, séparateurs français, identifiants/zéros, téléphones internationaux/multiples, conflits de segmentation, priorités individuelles, tri, annulation/rétablissement, résultat périmé, source intacte, export réouvrable et fichier verrouillé. Vérifier toutes les lignes, sans suppression implicite.
- Les trois familles métier et un fichier fictif volumineux ont servi à la recette initiale. Données réelles uniquement locales ; `scripts/acceptance.py` sert à une recette locale autorisée, jamais à envoyer des sources privées à la CI.
- **Actions d'abord** pour calculs lourds partageables et compilations : `.github/workflows/windows.yml` teste et construit ; `release.yml` se déclenche manuellement sur un tag déjà poussé, reconstruit la version étiquetée, vérifie portable/installation/désinstallation, puis publie les binaires et leurs SHA-256.
- Conditionnement validé : PyInstaller + Inno Setup. Configuration Nuitka conservée mais compilation complète non validée. Garder l'isolation de recherche des DLL dans le `.spec` : une bibliothèque ICU homonyme externe peut empêcher Qt de démarrer.
- Ne pas pousser classeurs réels, dictionnaires privés, bases de projets, journaux sensibles, environnements ou binaires. Pas d'identifiants d'accès dans le code ou les arguments de processus ; utiliser les mécanismes GitHub/Git prévus.
- Sources sur `main` par commits ciblés, sans réécriture forcée. Binaires uniquement dans les releases ; ne pas retélécharger/installer/lancer automatiquement chez l'utilisateur. Ne pas relancer un travail vérifié sans raison. La documentation seule ne nécessite pas de recompiler.

## 9. Reprise rapide par une IA

Lire `AGENTS.md`, ce mémo, puis seulement les modules concernés et leurs tests. Identifier la règle demandée et les invariants affectés ; distinguer état livré, demande historique et amélioration proposée. Corriger au bon niveau, tester sur données fictives, puis livrer via Actions si nécessaire. Actualiser ce mémo lorsqu'une décision durable change ; ne pas recopier les journaux ou tout l'historique de conversation.
