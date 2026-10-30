# FDX Atelier

**Préparation et contrôle de données Excel pour le suivi métier des marchés publics.**

FDX Atelier est une application Windows en français destinée aux équipes qui préparent des données administratives pour le contrôle de gestion, le reporting et l'analyse des marchés publics. Elle harmonise les fichiers selon un dictionnaire de données validé et conserve les anomalies qui nécessitent une décision humaine. Cette version utilise React et un moteur Rust local, sans Python embarqué.

[Versions vérifiées](https://github.com/AbouDjafar/FDX-Preprocessing-Tauri/releases) · [Aperçu de l'interface](https://aboudjafar.github.io/FDX-Preprocessing-Tauri/) · [Guide utilisateur](docs/GUIDE-TAURI.md) · [Transmission](docs/TRANSMISSION-TAURI.md)

**Version 2.1.0 en préparation pour Windows x64.** Sa publication exige les contrôles Rust/React et la validation Windows complète sur son commit exact. La dernière release publiée reste [2.0.0](https://github.com/AbouDjafar/FDX-Preprocessing-Tauri/releases/tag/v2.0.0), avec ses actifs inchangés : les [contrôles Rust et React](https://github.com/AbouDjafar/FDX-Preprocessing-Tauri/actions/runs/40359754161) et la [validation Windows complète](https://github.com/AbouDjafar/FDX-Preprocessing-Tauri/actions/runs/40359786062) avaient réussi sur le même commit. Ces contrôles utilisent des données fictives ; la validation métier des fichiers réels reste nécessaire. L'aperçu Pages utilise également des données fictives et ne traite aucun fichier personnel.

## Finalité métier

Le projet répond à des besoins spécifiques de préparation des données de **passation, de signature et d'exécution des marchés** : contrôler les champs, fiabiliser les formats et produire des classeurs cohérents pour leur exploitation métier.

Les règles dépendent du dictionnaire et du processus de l'organisation utilisatrice. L'application ne remplace ni un système de gestion des marchés, ni la validation des pièces sources, ni l'appréciation du responsable métier. Elle n'atteste pas la conformité réglementaire d'un marché.

## Fonctions principales

- Association des colonnes Excel aux champs attendus, avec confirmation des correspondances proposées.
- Contrôle des types, longueurs, champs obligatoires, valeurs autorisées et contraintes configurées.
- Traitement des nombres, dates, identifiants et téléphones ; segmentation des informations d'entreprise vers des colonnes existantes.
- Décisions explicites pour les cas ambigus, corrections annulables et contrôles chronologiques configurables.
- Export vérifié avec rapports facultatifs : `Contrôle`, `Nom_original` et `À vérifier`.
- Profils réutilisables et projets enregistrables pour reprendre un traitement.

## Démarrer

Windows x64 avec WebView2. L'exécutable ne nécessite ni Python, ni Node, ni Rust, ni Microsoft Excel. L'ancienne application et cette version peuvent coexister.

1. Télécharger manuellement l'installateur Windows x64 depuis les [releases vérifiées](https://github.com/AbouDjafar/FDX-Preprocessing-Tauri/releases). À partir de 2.1.0, seul l'installateur `FDX-Atelier-<version>-Windows-x64-installation.exe` sera distribué, accompagné du manifeste `BUILD.json` et de `SHA256SUMS.txt`. Aucune archive portable ne sera produite. L'installateur vérifie WebView2 et peut nécessiter Internet si ce composant manque. Les anciennes releases et leurs actifs restent inchangés.
2. Lancer l'application et choisir **Essayer un exemple** pour découvrir le fonctionnement sur des données fictives.
3. Choisir **Nouveau projet**, puis sélectionner un classeur `.xlsx` ou un modèle `.xltx` et son dictionnaire `.md` ou `.txt`. L’aperçu se consulte sur demande. Le nom et la famille du projet sont demandés lors de l’enregistrement.

| Étape | Décision attendue |
|---|---|
| Importer | Confirmer la feuille contenant les données et la ligne d'en-tête. |
| Associer | Valider le dictionnaire et les correspondances de colonnes. |
| Traiter | Choisir les formats, règles et décisions applicables. |
| Vérifier | Examiner les anomalies, préparer les corrections et vérifier le résultat. |
| Exporter | Choisir le nom et l'emplacement du fichier final, puis confirmer l'export. |

Les procédures détaillées figurent dans le [guide utilisateur](docs/GUIDE-TAURI.md).

## Données et confidentialité

- Les traitements des classeurs sont locaux, sans IA externe ni transfert des données vers un service distant.
- Le fichier source n'est pas remplacé ; les changements sont appliqués dans un fichier de sortie distinct.
- Les projets `.fdxproj` contiennent une copie des données et du dictionnaire. Les projets, profils et résultats doivent être conservés et partagés selon leur niveau de confidentialité.
- Les tests automatisés utilisent des données fictives. Les données métier réelles, dictionnaires privés et journaux sensibles ne doivent pas être publiés dans le dépôt ou les issues.

## Garanties et limites

L'analyse porte sur toutes les lignes de la feuille choisie. Aucune ligne n'est supprimée automatiquement. L'export impose une rotation de texte nulle et des nombres sans séparateur de milliers ; les identifiants doivent rester textuels pour préserver leurs préfixes et zéros initiaux.

Les numéros à supprimer, dates impossibles, troncatures et cellules à vider nécessitent une règle ou une décision explicite. Une proposition de correction ne constitue pas une preuve de la valeur réelle.

La version actuelle traite un fichier et une feuille de données à la fois, jusqu'à cinq millions de cellules dans la feuille traitée. Les formats `.xls`, macros, classeurs chiffrés et certains objets Excel avancés ne sont pas pris en charge. Les objets incompatibles détectés sont refusés pour limiter les risques de perte. L'exécutable n'est pas signé : Windows peut afficher un avertissement de réputation.

## Développement et livraison

Le moteur Rust indépendant se trouve dans `crates/fdx-core`, la CLI dans `crates/fdx-cli`, l'hôte natif dans `src-tauri` et le frontend dans `web`. Les dépendances sont verrouillées par `Cargo.lock` et `web/package-lock.json`. Le code Python historique reste uniquement une référence de comparaison pour les tests ; il n'est pas livré dans l'application Tauri.

Les installations de dépendances, tests complets, compilations et conditionnements passent exclusivement par [GitHub Actions](https://github.com/AbouDjafar/FDX-Preprocessing-Tauri/actions). Les exécutables et installateurs sont distribués dans les releases, jamais dans l'historique Git. La publication réutilise un artefact vérifié au même commit, sans recompilation.

La version de `web/package.json` sert au frontend et aux noms de livraison ; les versions Cargo, les lockfiles et la configuration native doivent correspondre avant compilation. Les licences tierces sont préparées dans `qa/licenses` sur le runner, incluses dans NSIS et vérifiées après installation. La publication exige un tag de version au commit exact du run Windows et des contrôles Rust/React réussis, puis vérifie la provenance et les empreintes de l'installateur et du manifeste. Elle refuse tout actif supplémentaire, notamment portable, et ne remplace jamais une release existante. Les noms techniques du dépôt, des modules et de l'exécutable, ainsi que l'identifiant de l'application, restent stables.

Le changement de nom visible inclut une migration de l'ancienne inscription NSIS en mode mise à jour, sans demander la suppression des données de l'application. Le workflow Windows vérifie ce chemin avec une inscription fictive utilisant le nouvel installateur ; il ne télécharge ni ne modifie une ancienne release.

Pour reprendre le développement, lire les [instructions du dépôt](AGENTS.md), puis la [transmission Tauri](docs/TRANSMISSION-TAURI.md). Le [mémo historique](docs/TRANSMISSION.md) décrit les règles métier d'origine et la version Python, pas l'architecture actuelle. Ce dépôt dérive de [FDX-Preprocessing](https://github.com/AbouDjafar/FDX-Preprocessing) et conserve son historique.

## Signaler un problème

Utiliser les [issues du dépôt](https://github.com/AbouDjafar/FDX-Preprocessing-Tauri/issues) en indiquant la version, les étapes de reproduction et le résultat attendu. Joindre uniquement un exemple fictif ou anonymisé ; retirer des captures les informations personnelles, chemins locaux et données confidentielles.
