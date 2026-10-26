import { expect, test, type Locator, type Page } from '@playwright/test';

async function noHorizontalOverflow(page: Page) {
  const geometry = await page.evaluate(() => ({
    viewport: window.innerWidth,
    document: document.documentElement.scrollWidth,
    bodyRight: document.body.getBoundingClientRect().right,
  }));
  expect(geometry.document, JSON.stringify(geometry)).toBeLessThanOrEqual(geometry.viewport + 1);
  expect(geometry.bodyRight, JSON.stringify(geometry)).toBeLessThanOrEqual(geometry.viewport + 1);
}

async function usableControl(control: Locator, page: Page) {
  await control.scrollIntoViewIfNeeded();
  await expect(control).toBeVisible();
  await expect(control).toBeEnabled();
  await expect(control).toBeInViewport({ ratio: 0.99 });
  const bounds = await control.boundingBox();
  expect(bounds).not.toBeNull();
  const viewport = page.viewportSize()!;
  expect(bounds!.x).toBeGreaterThanOrEqual(-1);
  expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(viewport.width + 1);
  expect(bounds!.width).toBeGreaterThan(16);
  expect(bounds!.height).toBeGreaterThan(16);
  // Trial clicks also reject controls obscured by sticky surfaces or another element.
  await control.click({ trial: true });
  await noHorizontalOverflow(page);
}

test('rules and review controls fit the display, DPR and logical zoom', async ({ page }, info) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('/');
  const cssZoom = Number(info.project.metadata.cssZoom || 1);
  // Pixel density alone is not Windows scaling: this case additionally shrinks usable layout space.
  if (cssZoom !== 1) await page.evaluate(zoom => document.body.style.setProperty('zoom', String(zoom)), cssZoom);
  expect(await page.evaluate(() => window.devicePixelRatio)).toBeCloseTo(info.project.use.deviceScaleFactor ?? 1);
  await page.getByTestId('try-demo').click();
  await page.getByTestId('stage-association').click();
  await page.getByRole('button', { name: 'Confirmer toutes les associations', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Confirmer toutes les associations', exact: true }).click();
  await page.getByRole('button', { name: 'Enregistrer et définir les règles', exact: true }).click();

  await page.getByRole('button', { name: 'Avancé', exact: true }).click();
  await expect(page.getByLabel('Champ métier', { exact: true })).toHaveAccessibleName('Champ métier');
  await page.getByLabel('Champ métier', { exact: true }).selectOption('TELEPHONE');
  await page.evaluate(async () => { await document.fonts.ready; });
  await usableControl(page.getByRole('button', { name: 'Charger un profil', exact: true }), page);
  await usableControl(page.getByRole('button', { name: 'Enregistrer le profil', exact: true }), page);
  await usableControl(page.getByLabel('Champ métier', { exact: true }), page);
  await page.screenshot({ path: info.outputPath('rules-display.png') });
  await usableControl(page.getByLabel('Numéros multiples', { exact: true }), page);
  await usableControl(page.getByRole('button', { name: 'Ajouter une relation', exact: true }), page);
  const analyze = page.getByRole('button', { name: 'Confirmer et analyser toutes les lignes', exact: true });
  await usableControl(analyze, page);
  await page.screenshot({ path: info.outputPath('rules-primary-control.png') });
  await analyze.click();

  await expect(page.getByRole('heading', { name: 'Examiner les résultats', exact: true })).toBeVisible();
  await expect(page.getByTestId('finding').first()).toBeVisible();
  await expect(page.getByTestId('task-progress')).toHaveCount(0);
  await expect(page.locator('.toast')).toHaveCount(0);
  await usableControl(page.getByRole('button', { name: 'Analyser à nouveau', exact: true }), page);
  await usableControl(page.getByLabel('Rechercher dans les résultats', { exact: true }), page);
  await usableControl(page.getByLabel('Type d’anomalie', { exact: true }), page);
  await page.screenshot({ path: info.outputPath('review-display.png') });
  const decide = page.getByTestId('finding').first().getByRole('button', { name: 'Examiner et décider', exact: true });
  await usableControl(decide, page);
  await page.screenshot({ path: info.outputPath('review-decision-control.png') });
  await decide.click();
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeInViewport({ ratio: 0.99 });
  const dialogBounds = await dialog.boundingBox();
  expect(dialogBounds!.height).toBeLessThanOrEqual(page.viewportSize()!.height);
  await expect(dialog.getByLabel('Décision', { exact: true })).toHaveAccessibleName('Décision');
  await usableControl(dialog.getByLabel('Décision', { exact: true }), page);
  await usableControl(dialog.getByRole('button', { name: 'Ajouter la décision au lot', exact: true }), page);
  await page.screenshot({ path: info.outputPath('review-decision-dialog.png') });
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await usableControl(page.getByLabel('Résultats par page', { exact: true }), page);
  await usableControl(page.getByRole('button', { name: 'Page suivante', exact: true }), page);
  await usableControl(page.getByRole('button', { name: 'Préparer l’export', exact: true }), page);
  await page.screenshot({ path: info.outputPath('review-primary-control.png') });
  expect(errors).toEqual([]);
});
