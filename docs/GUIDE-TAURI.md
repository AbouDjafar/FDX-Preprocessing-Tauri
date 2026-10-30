# FDX Atelier

Application locale de preparation de classeurs de suivi des marches publics : passation, marches signes et execution. Elle aide a verifier des donnees selon un dictionnaire metier ; elle ne certifie pas la regularite juridique d'un marche.

## Installation

Telecharger manuellement l'installateur Windows x64 dans les releases du depot. A partir de la version 2.1.0, seul l'installateur est distribue, accompagne de `BUILD.json` et `SHA256SUMS.txt` ; aucune archive portable n'est produite. Les actifs des anciennes releases restent inchanges. Le manifeste indique la version, le commit exact verifie et le run Actions ; les empreintes couvrent le manifeste et l'installateur.

L'installateur utilise le runtime WebView2 present et propose son installation s'il manque ; cette derniere peut necessiter Internet. Python, Node et Rust ne sont pas necessaires. Les licences tierces et ce guide sont inclus dans l'installation. Les binaires ne sont pas signes avec un certificat commercial ; Windows peut demander une confirmation de provenance.

La demonstration GitHub Pages utilise exclusivement des exemples fictifs. Elle ne lit pas les classeurs personnels et ne produit pas de fichier Excel traite. Utiliser l'application Windows pour les vrais fichiers.

## Parcours

1. Choisir Nouveau projet pour entrer directement dans l'import, ou essayer l'exemple fictif. Importer un `.xlsx` ou un modele `.xltx` et un dictionnaire `.md` ou `.txt`. Confirmer la feuille et la ligne d'en-tete. Le bouton Voir l'apercu ouvre l'echantillon ; il reste masque tant que ce bouton n'est pas utilise.
2. Verifier les champs et les associations de colonnes. Les suggestions doivent etre confirmees. Les colonnes ignorees conservent leurs valeurs.
3. Choisir les regles : separateurs des nombres, ordre des dates, telephones, remplacements, segmentation et chronologie. Les reglages avances partagent les memes parametres que le parcours guide.
4. Analyser toutes les lignes. Examiner les anomalies et les changements prepares. Une decision peut conserver, corriger ou vider une cellule ; elle ne supprime jamais la ligne. Verifier les corrections apres les decisions, y compris apres annulation ou retablissement.
5. Choisir le nom, le dossier et les rapports. Confirmer explicitement les anomalies restantes et tout remplacement de fichier. L'application rouvre et verifie le classeur temporaire avant de publier le resultat.

## Decisions importantes

- Telephones multiples : conserver/signaler par defaut ; choisir un numero, garder le premier ou le premier valide, garder tous ou vider sont des choix explicites. Une validation de format ne prouve pas que le numero appartient a l'entreprise.
- Dates : ne pas corriger une annee ou une chronologie par intuition. Une date impossible reste signalee. Les relations entre colonnes peuvent etre desactivees.
- Identifiants : les conserver en texte ; des zeros deja perdus dans une source numerique ne peuvent pas etre inventes.
- Segmentation : choisir les colonnes existantes pour entreprise, BP, telephone et localisation. Les conflits de destination demandent une decision.
- Nombres : aucune suppression arbitraire de decimales. Les nombres dont la precision ne peut pas etre conservee dans Excel peuvent bloquer l'export.

## Sauvegarde et rapports

Un profil JSON reutilise des regles sans embarquer les fichiers sources. Ses associations doivent etre reconfirmees. Un projet `.fdxproj` conserve une copie des sources, les parametres et les decisions ; ce fichier contient donc des donnees confidentielles, non chiffrees. Garder les projets dans un emplacement adapte.

Le nom et la famille de donnees sont demandes lors de l'enregistrement, pas au demarrage. Avant de fermer l'application ou de remplacer un travail non enregistre, choisir Enregistrer et continuer, Continuer sans enregistrer ou Annuler. Les choix d'import restent disponibles apres un retour a l'accueil ; les confirmer avec Importer et associer avant d'enregistrer le projet.

Les anciens projets v1 sont lus sans etre modifies et necessitent une nouvelle analyse. Choisir un nouveau nom pour les sauvegarder au format actuel. Les resultats caches de l'ancienne version ne sont pas consideres comme une verification Rust. Le nom visible FDX Atelier ne change ni l'identifiant de l'application ni le format des projets.

`Controle` contient le dictionnaire et les indicateurs, `Nom_original` les valeurs avant modification, `A verifier` les anomalies encore presentes. Leurs noms dans Excel conservent les accents. Des suffixes preservent les feuilles homonymes existantes, sauf remplacement explicitement autorise.

Sans rapports, la structure source est conservee. Dans tous les cas, l'orientation devient horizontale et les formats numeriques perdent le groupement des milliers. Cela ne signifie pas aligner tous les nombres a gauche.

## Incidents

Une source modifiee exige une nouvelle importation. Un resultat perime exige une nouvelle analyse. Pour un fichier de sortie verrouille, le fermer dans Excel ou choisir un autre nom. Une annulation ou un echec de verification ne doit pas publier un fichier incomplet.

Les formats a macros, fichiers binaires `.xls`/`.xlsb`, classeurs chiffres et objets avances non conservables sont refuses. Une fenetre explique la cause et les actions possibles : ouvrir une copie dans Excel ou LibreOffice, la reparer si necessaire, verifier les donnees et objets conserves, enregistrer puis recharger la copie. Ne jamais supprimer des macros utiles sans decision explicite ni convertir en renommant seulement l'extension. La sortie des sources `.xlsx` et `.xltx` est un classeur `.xlsx` verifie.
