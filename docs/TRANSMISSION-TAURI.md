# Transmission Tauri

## Etat et objectif

Migration vers 2.0.0 : React/TypeScript, Tauri 2, moteur entierement Rust. Le code Python conserve sert d'oracle de test, jamais de runtime distribue. Ne pas confondre presence de code, build reussi et release validee. Lire les resultats Actions du commit concerne avant toute affirmation de recette.

Finalite : preparation de classeurs de passation/signature/execution des marches publics selon un dictionnaire confirme. Aucune certification juridique ; source et decisions metier font autorite. Une intuition n'autorise pas une correction.

## Carte et contrats

- `crates/fdx-core` : modeles, dictionnaire, transformations, analyse, lecture/ecriture Excel, stockage et exemple fictif. Bibliotheque sans Tauri.
- `crates/fdx-cli` : inspect, analyze, export, selftest. Ne pas exposer le moteur Python dans le livrable.
- `src-tauri/src/service.rs` : sessions versionnees, revisions, decisions en lot, historique, catalogues locaux et commandes. Traitements hors thread UI ; annulation independante du verrou de session.
- `web/src` : parcours en cinq etapes, service natif ou demonstration explicite, erreurs et chargement. Un echec natif ne doit jamais basculer silencieusement vers des donnees fictives.
- `docs/TAURI-CONTRACT.md` : schemas et noms d'operations. `CellValue={kind,value}` utilise des chaines pour les nombres exacts et les identifiants. Indices Excel 1-based, cle `feuille!ligne:colonne` independante du tri.

## Invariants metier

Analyser toutes les lignes apres l'en-tete. Valider les associations, y compris les en-tetes identiques. Conserver les colonnes ignorees. Decimales exactes pendant l'analyse, pas d'arrondi implicite. Telephones et identifiants en texte ; aucune invention de zeros perdus. Dates impossibles et ambiguite restent signalees. Chronologie seulement si configuree. Segmentation vers des colonnes existantes ; conflit preserve sans autorisation. Decision individuelle prioritaire ; vider n'est jamais supprimer une ligne. Conserver une anomalie ne la rend pas conforme.

Resultat courant = source + configuration + decisions correspondantes. Une nouvelle configuration/revision invalide le resultat concerne. Annuler/retablir une decision exige un nouveau calcul avant export. Formules conservees ; textes commencant par `=` ne doivent pas devenir des formules.

## Excel et stockage

Lecture raw XML bornee et validation umya-spreadsheet. Ecriture conservatrice du paquet OOXML ; conserver les parties source non modifiees. Refuser objets non conservables, entites/DTD, archives hors limites et precision non conservable. Valeurs, formules, feuilles, fusions, dimensions, filtres et volets doivent etre verifies.

Rotation zero et formats sans groupement sur cellules, lignes, colonnes et styles conditionnels. Preserver pourcentages, valeurs, decimales et dates. Rapports selectionnables control/original/verify, suffixes si noms deja presents. Rapports reconstruits depuis le resultat final. Temporaire dans le dossier cible, reouverture/verification, empreinte source finale, publication atomique. Jamais ecraser source ou dictionnaire.

Profils JSON sans chemins personnels ; mappings reconfirmes. Projets SQLite contiennent copie source, dictionnaire, decisions et historique, non chiffres. Import v1 vers v2 sans remplacement silencieux de l'ancien fichier et sans reutilisation de l'ancienne analyse. Les projets/caches/catalogues sont locaux, jamais dans Git ou les artefacts publics.

## Actions uniquement

- `bootstrap.yml` : resout/verrouille les dependances et peut appliquer rustfmt sur serveur, puis commit de sources/lockfiles uniquement. Recuperer ce commit avant de pousser d'autres changements. Aucun package a installer localement.
- `checks.yml` : format, lint, tests Rust/React, oracle Python sur donnees fictives, verification Excel independante et Playwright.
- `pages.yml` : frontend statique, donnees fictives, routage hash et assets locaux. Aucun traitement de fichiers personnels.
- `windows-tauri.yml` : tests Windows, compilation release/NSIS, tests du vrai pont natif via WebDriver, parcours visuels, installation/desinstallation et artefacts verifies. Declenchement manuel.
- `publish.yml` : publication sans reconstruire, uniquement pour un run Windows reussi et un commit ayant aussi passe checks. Tag exact obligatoire, manifeste et SHA verifies. Pas d'ecrasement d'une release existante.

Les binaires sont telecharges manuellement par l'utilisateur. Ne jamais telecharger/installer/lancer une release sur son PC sans nouvelle demande. Aucun classeur prive sur les runners. Aucun chemin personnel, token ou journal sensible dans docs/issues. Le serveur de developpement Playwright existe uniquement dans la CI, jamais dans l'application distribuee.

## Reprise rapide

Lire ce memo puis uniquement les modules concernes. Examiner `git status` sans annuler des changements existants. Verifier source/oracle et tests avant de modifier une regle. Les tests de conservation vivent dans `crates/fdx-core/tests/io.rs`, metier dans `business.rs`, integration native dans `scripts/native_smoke.py`, parite dans `scripts/compare_engines.py`. Maintenir les trois contrats frontend/natif/core ensemble. La garantie de conservation prime sur un export qui semble fonctionner.
