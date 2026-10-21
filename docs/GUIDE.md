# Guide utilisateur

## Commencer

Ouvrir FDX Atelier Données. Le mode guidé est actif par défaut. Choisir les deux fichiers ou les déposer dans l'écran Mes fichiers. Le dictionnaire décrit les colonnes attendues, leurs types et les contraintes.

Vérifier l'aperçu : les noms de colonnes doivent correspondre à la ligne d'en-tête choisie. La feuille et l'en-tête proposés ne sont que des suggestions ; confirmer avant de poursuivre. Le fichier source n'est jamais remplacé.

## Confirmer les colonnes

Une ligne correspond à une position dans Excel, même si deux colonnes portent le même nom. Contrôler le champ attendu et le format. « Ignorer » conserve les valeurs de la colonne sans appliquer de règle métier ; les garanties de présentation restent globales.

Le bouton Confirmer les colonnes accepte les associations affichées, puis ouvre les traitements. Le bouton Analyser le fichier examine toutes les lignes, pas seulement l'aperçu.

Les téléphones et identifiants sont normalement du texte, même s'ils contiennent des chiffres. Cela protège les zéros et les préfixes. Les nombres ont des séparateurs d'entrée configurables ; aucun séparateur de milliers n'est ajouté en sortie.

## Décider des corrections

Les trois vues ont des rôles distincts :

- **Problèmes à traiter** : situations ambiguës ou incompatibles avec le dictionnaire.
- **Modifications préparées** : conversions déjà proposées par les paramètres ; sélectionner Conserver la valeur d'origine pour en annuler une.
- **Historique** : décisions et opérations d'annulation de la session.

Dans la vue guidée, utiliser les flèches pour parcourir les problèmes et le filtre pour choisir une catégorie. Comparer l'origine et la proposition, choisir une action et préparer la correction. Une correction préparée n'est pas encore validée : le bouton Vérifier mes corrections recalcule le lot. Un nouveau contrôle peut révéler une autre contrainte sur la même cellule.

Pour plusieurs téléphones, choisir un numéro précis, le premier, le premier valide pour le pays retenu, tous les numéros avec séparateur, ou vider la cellule. La reconnaissance d'un numéro ne prouve pas qu'il appartient à l'entreprise.

Une date impossible n'est jamais corrigée silencieusement. Les propositions sont des interprétations possibles, pas une preuve de la date réelle. Une incohérence chronologique ne justifie pas automatiquement de changer l'année.

« Conserver et signaler » ne supprime pas une anomalie réelle. Elle restera dans À vérifier. Vider une cellule ne supprime jamais la ligne.

## Réglages avancés

Activer Mode avancé pour éditer le dictionnaire et travailler sur un tableau de problèmes filtrable, triable et à sélection multiple. Les décisions utilisent toujours les coordonnées d'origine, indépendamment du tri.

Dans le dictionnaire : longueur maximale, format date, caractère obligatoire, valeurs autorisées (une par ligne), minimum, maximum et motif de conformité. Les motifs sont des expressions régulières, jamais du code exécuté.

Dans Traitements : remplacements exacts via un tableau de valeurs ; séparateurs numériques ; pays des téléphones ; relations entre dates ; séparation entreprise/BP/téléphone/localisation. Choisir explicitement les colonnes de destination. Les conflits sont conservés et signalés, sauf remplacement volontairement activé.

Un réglage modifié rend l'analyse précédente obsolète. Une nouvelle analyse est obligatoire avant l'export. Le mode guidé et le mode avancé partagent exactement les mêmes réglages et décisions.

## Enregistrer et reprendre

Choisir le nom complet du résultat avec Choisir. Ajouter les rapports ou conserver la seule structure source. Les anciens rapports sont conservés et suffixés par défaut ; leur remplacement demande une confirmation. L'export avec problèmes restants nécessite une confirmation explicite.

Après vérification du fichier final, Ouvrir le fichier et Ouvrir le dossier donnent accès au résultat.

Le menu Projet permet d'enregistrer une session `.fdxproj` et de la reprendre plus tard. Elle contient les sources, réglages et décisions. Un profil JSON contient les réglages réutilisables, mais pas les données : ses correspondances doivent être confirmées pour chaque fichier.

Les boutons d'annulation/rétablissement concernent les décisions de correction, pas la suppression de fichiers. Fermer un projet modifié propose son enregistrement.

## Difficultés courantes

- **Fichier verrouillé** : fermer le résultat dans Excel ou choisir un autre nom.
- **Objet Excel non pris en charge** : utiliser une copie tabulaire sans cet objet ; ne pas modifier l'original pour contourner le contrôle.
- **Aucune correspondance** : vérifier la ligne d'en-tête et le dictionnaire, puis choisir les champs manuellement.
- **Problème toujours présent après correction** : vérifier les autres contraintes du dictionnaire. Une cellule peut cumuler plusieurs problèmes.
- **Écran réduit ou zoom Windows élevé** : faire défiler les formulaires ; la commande principale reste en bas de la fenêtre.

Formats acceptés : `.xlsx`, `.md`, `.txt`. Les macros, anciens fichiers `.xls`, classeurs chiffrés et traitements par lots ne font pas partie de cette version.
