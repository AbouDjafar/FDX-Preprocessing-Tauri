# FDX Atelier Données

**Préparation et contrôle de données Excel pour le suivi métier des marchés publics.**

FDX Atelier Données est une application Windows en français destinée aux équipes qui préparent des données administratives pour le contrôle de gestion, le reporting et l'analyse des marchés publics. Elle harmonise les fichiers selon un dictionnaire de données validé et conserve les anomalies qui nécessitent une décision humaine.

[Télécharger une version](https://github.com/AbouDjafar/FDX-Preprocessing/releases) · [Guide utilisateur](docs/GUIDE.md) · [Transmission du projet](docs/TRANSMISSION.md)

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

Windows x64. L'exécutable ne nécessite ni Python ni Microsoft Excel.

1. Télécharger manuellement l'installateur ou l'archive portable depuis les [releases](https://github.com/AbouDjafar/FDX-Preprocessing/releases). Pour la version portable, extraire l'ensemble de l'archive et conserver le dossier `_internal` avec l'exécutable.
2. Lancer l'application et choisir **Essayer un exemple** pour découvrir le fonctionnement sur des données fictives.
3. Pour un traitement métier, sélectionner un classeur `.xlsx` et son dictionnaire `.md` ou `.txt`, puis suivre les quatre étapes.

| Étape | Décision attendue |
|---|---|
| Mes fichiers | Confirmer la feuille contenant les données et la ligne d'en-tête. |
| Préparer | Valider les associations de colonnes, les formats et les règles applicables. |
| Vérifier | Examiner les anomalies, préparer les corrections et vérifier le résultat. |
| Enregistrer | Choisir le nom et l'emplacement du fichier final, puis confirmer l'export. |

Les procédures détaillées figurent dans le [guide utilisateur](docs/GUIDE.md).

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

Le moteur Python est séparé de l'interface. Les dépendances et versions de référence sont décrites dans [pyproject.toml](pyproject.toml) et [requirements-lock.txt](requirements-lock.txt).

Les tests complets et compilations Windows passent prioritairement par [GitHub Actions](https://github.com/AbouDjafar/FDX-Preprocessing/actions). Les exécutables et installateurs sont distribués dans les releases, jamais dans l'historique Git. Les composants tiers et leurs licences accompagnent la distribution.

Pour reprendre le développement, lire les [instructions du dépôt](AGENTS.md), puis le [document de transmission](docs/TRANSMISSION.md), qui décrit les règles métier, contrats techniques, limites et vérifications à préserver.

## Signaler un problème

Utiliser les [issues du dépôt](https://github.com/AbouDjafar/FDX-Preprocessing/issues) en indiquant la version, les étapes de reproduction et le résultat attendu. Joindre uniquement un exemple fictif ou anonymisé ; retirer des captures les informations personnelles, chemins locaux et données confidentielles.
