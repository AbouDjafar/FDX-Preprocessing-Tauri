import { expect, test } from '@playwright/test';

test('starts immediately and asks project details only when saving', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByTestId('app-version')).toHaveText('v2.1.0');
  await page.getByTestId('create-project').click();
  await expect(page.getByRole('heading', { name: 'Importer les sources', exact: true })).toBeVisible();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.getByRole('button', { name: 'Choisir l’exemple', exact: true }).click();
  await page.getByRole('button', { name: 'Importer et associer', exact: true }).click();
  await page.getByTestId('save-project').click();
  const details = page.getByRole('dialog', { name: 'Enregistrer le projet', exact: true });
  await expect(details).toBeVisible();
  await details.getByLabel('Nom du projet', { exact: true }).fill('Contrôle préparé');
  await details.getByLabel('Famille de données', { exact: true }).fill('Passation');
  await details.getByRole('button', { name: 'Choisir l’emplacement', exact: true }).click();
  await expect(page.getByText('Projet DEMO enregistré en mémoire. Aucun fichier créé.', { exact: true })).toBeVisible();
  await expect(page.locator('.crumb')).toContainText('Contrôle préparé');
  await page.getByRole('button', { name: 'Vue d’ensemble', exact: true }).click();
  await expect(page.locator('.project-row').first()).toContainText('Contrôle préparé');
  await expect(page.locator('.project-row').first()).toContainText('Passation');
});

test('can cancel or save unsaved work before replacing a project', async ({ page }) => {
  await page.goto('/'); await page.getByTestId('try-demo').click();
  await page.getByRole('button', { name: 'Nouveau projet', exact: true }).click();
  const warning = page.getByRole('dialog', { name: 'Enregistrer avant de continuer ?', exact: true });
  await expect(warning).toBeVisible();
  await warning.getByRole('button', { name: 'Annuler', exact: true }).click();
  await expect(page.locator('.project-mini')).toContainText('Marchés fictifs');
  await page.getByRole('button', { name: 'Nouveau projet', exact: true }).click();
  await warning.getByRole('button', { name: 'Enregistrer et continuer', exact: true }).click();
  const details = page.getByRole('dialog', { name: 'Enregistrer le projet', exact: true });
  await details.getByLabel('Nom du projet', { exact: true }).fill('Exemple conservé');
  await details.getByRole('button', { name: 'Choisir l’emplacement', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Importer les sources', exact: true })).toBeVisible();
  await expect(page.locator('.project-mini')).toContainText('Projet sans titre');
  await page.getByRole('button', { name: 'Vue d’ensemble', exact: true }).click();
  await expect(page.locator('.project-row').first()).toContainText('Exemple conservé');
});

test('resets scrolling between stages and keeps the stage header available', async ({ page }, info) => {
  await page.goto('/'); await page.getByTestId('try-demo').click();
  await page.getByTestId('stage-traitements').click();
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await expect(page.locator('.stepper')).toBeInViewport();
  await page.getByTestId('stage-association').click();
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(0);
  await page.getByRole('button', { name: 'Étape suivante', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Confirmer les règles', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Étape précédente', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Associer les colonnes', exact: true })).toBeVisible();
  await page.screenshot({ path: info.outputPath('navigation-2.1.png'), fullPage: true });
});
