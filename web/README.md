# FDX Atelier

Prototype web en français pour parcourir une interface de préparation et de
vérification de classeurs de marchés publics.

## Lancer l’application

Prérequis : Node.js 20 ou version ultérieure.

```bash
npm install
npm run dev
```

Pour créer une version optimisée :

```bash
npm run build
```

## Limites de cette maquette

Les projets et décisions utilisent des exemples fictifs enregistrés dans le
navigateur. La sélection d'un fichier affiche son nom, mais ne lit pas le
classeur ni son dictionnaire. L'analyse métier réelle et la génération d'un
fichier Excel ne sont pas incluses.
