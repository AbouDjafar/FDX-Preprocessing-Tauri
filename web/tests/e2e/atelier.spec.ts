import { expect, test, type Page } from '@playwright/test';

async function demo(page: Page) {
  await page.goto('/');
  await page.getByTestId('try-demo').click();
  await expect(page.getByRole('heading', { name: 'Importer les sources', exact: true })).toBeVisible();
}
async function analyzed(page: Page) {
  await demo(page);
  await page.getByTestId('stage-association').click();
  await page.getByRole('button', { name: 'Confirmer toutes les associations', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Confirmer toutes les associations', exact: true }).click();
  await page.getByRole('button', { name: 'Enregistrer et définir les règles' }).click();
  await page.getByRole('button', { name: 'Confirmer et analyser toutes les lignes' }).click();
  await expect(page.getByRole('heading', { name: 'Examiner les résultats', exact: true })).toBeVisible();
  await expect(page.getByTestId('finding').first()).toBeVisible();
}
async function noHorizontalOverflow(page: Page) {
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)).toBe(true);
}

test('complete synthetic journey, precise decision, undo/redo and explicit non-Excel receipt', async ({ page }, info) => {
  const errors: string[] = []; page.on('pageerror', e => errors.push(e.message));
  await analyzed(page);
  await noHorizontalOverflow(page);
  expect(await page.getByTestId('finding').count()).toBeLessThan(40);
  await page.screenshot({ path: info.outputPath('review-initial.png'), fullPage: true });
  const first = page.getByTestId('finding').first();
  await expect(first).toContainText('Marchés fictifs!2:2');
  await first.getByRole('button', { name: 'Examiner et décider' }).click();
  const dialog = page.getByRole('dialog');
  const decision = dialog.getByLabel('Décision', { exact: true });
  await expect(decision).toHaveAccessibleName('Décision');
  await decision.selectOption('manual');
  await expect(decision).toHaveAccessibleName('Décision');
  await dialog.getByLabel('Type de la valeur', { exact: true }).selectOption('decimal');
  const manualValue = dialog.getByLabel('Valeur manuelle', { exact: true });
  await expect(manualValue).toHaveAccessibleName('Valeur manuelle');
  await expect(manualValue).toHaveAccessibleDescription('Point décimal, sans groupement. Précision conservée.');
  await dialog.locator('label').filter({ hasText: /^Valeur manuelle$/ }).click();
  await expect(manualValue).toBeFocused();
  await manualValue.fill('12345678901234567890.123456789');
  await expect(dialog.getByLabel('Motif de la décision', { exact: true })).toHaveAccessibleName('Motif de la décision');
  await dialog.getByRole('button', { name: 'Ajouter la décision au lot' }).click();
  await expect(page.getByRole('button', { name: 'Vérifier le lot', exact: true })).toBeVisible();
  await page.getByTestId('stage-export').click();
  const reportToggle = page.getByLabel('Contrôle', { exact: true });
  await expect(reportToggle).toHaveAccessibleName('Contrôle');
  await expect(reportToggle).toHaveAccessibleDescription('Dictionnaire et indicateurs des champs associés');
  await expect(page.getByTestId('demo-receipt')).toBeDisabled();
  await expect(page.getByText('Analyse à actualiser.', { exact: true })).toBeVisible();
  await page.getByTestId('stage-anomalies').click();
  await page.getByRole('button', { name: 'Vérifier le lot', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Préparer l’export', exact: true })).toBeEnabled();
  await page.getByRole('tab', { name: 'Changements' }).click();
  await expect(page.getByRole('tabpanel')).toContainText('12345678901234567890.123456789');
  await page.getByRole('button', { name: 'Annuler le dernier lot de décisions', exact: true }).click();
  await page.getByRole('button', { name: 'Vérifier le lot', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Aucun résultat pour cette vue' })).toBeVisible();
  await page.getByRole('button', { name: 'Rétablir le dernier lot de décisions', exact: true }).click();
  await page.getByRole('button', { name: 'Vérifier le lot', exact: true }).click();
  await expect(page.getByRole('tabpanel')).toContainText('12345678901234567890.123456789');
  await page.getByRole('tab', { name: 'Historique' }).click();
  await expect(page.getByRole('tabpanel')).toContainText('decide');
  await page.getByTestId('stage-export').click();
  await page.getByLabel('J’autorise l’export avec les anomalies restantes, sans les déclarer conformes.').check();
  const downloaded = page.waitForEvent('download'); await page.getByTestId('demo-receipt').click();
  const file = await downloaded; expect(file.suggestedFilename()).toBe('FDX-DEMO-releve-non-excel.json');
  await file.saveAs(info.outputPath('DEMO-receipt.json'));
  await expect(page.getByText('Relevé DEMO généré. Aucun fichier .xlsx n’a été créé ni vérifié.')).toBeVisible();
  await noHorizontalOverflow(page);
  await page.screenshot({ path: info.outputPath('export-demo.png'), fullPage: true });
  expect(errors).toEqual([]);
});

test('editable dictionaries, distinct column positions, rules, profiles and project reopen', async ({ page }, info) => {
  await demo(page); await page.getByTestId('stage-association').click();
  await expect(page.getByLabel('Champ colonne D', { exact: true })).toBeVisible();
  await expect(page.getByLabel('Champ colonne K', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Modifier MONTANT', exact: true }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByText('Contraintes avancées', { exact: true }).click();
  await dialog.getByLabel('Maximum (décimal exact)').fill('900719925474099312345.9876');
  await dialog.getByRole('button', { name: 'Enregistrer le champ', exact: true }).click();
  await expect(page.getByText('Max. 900719925474099312345.9876', { exact: true })).toBeVisible();
  await page.getByTestId('stage-traitements').click();
  await page.getByRole('button', { name: 'Avancé', exact: true }).click();
  await page.getByLabel('Champ métier', { exact: true }).selectOption('TELEPHONE');
  await expect(page.getByLabel('Champ métier', { exact: true })).toHaveAccessibleName('Champ métier');
  await page.getByLabel('Numéros multiples', { exact: true }).selectOption('first_valid');
  await expect(page.getByLabel('Extraire les informations du contact', { exact: true })).toHaveAccessibleName('Extraire les informations du contact');
  await page.getByLabel('Extraire les informations du contact', { exact: true }).check();
  await page.getByLabel('Colonne source', { exact: true }).selectOption('5');
  await page.getByLabel('Destination entreprise', { exact: true }).selectOption('9');
  await page.getByLabel('Destination BP', { exact: true }).selectOption('10');
  await page.getByLabel('Destination téléphone', { exact: true }).selectOption('11');
  await page.getByLabel('Destination localisation', { exact: true }).selectOption('12');
  await page.getByRole('button', { name: 'Ajouter une relation', exact: true }).click();
  await page.getByLabel('Date antérieure ou égale').selectOption('SIGNATURE');
  await page.getByLabel('Date postérieure ou égale').selectOption('FIN');
  await page.getByRole('button', { name: 'Enregistrer le profil', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Profil DEMO conservé' })).toBeVisible();
  await page.getByRole('button', { name: 'Charger un profil', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Charger le profil', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Confirmer et analyser toutes les lignes' })).toBeDisabled();
  await page.getByTestId('save-project').click();
  await expect(page.getByText('Projet DEMO enregistré en mémoire. Aucun fichier créé.', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Vue d’ensemble', exact: true }).click();
  await page.locator('.project-row').first().click();
  await expect(page.getByRole('heading', { name: 'Confirmer les règles', exact: true })).toBeVisible();
  await expect(page.getByLabel('Destination entreprise', { exact: true })).toHaveValue('9');
  await noHorizontalOverflow(page); await page.screenshot({ path: info.outputPath('rules-and-profile.png'), fullPage: true });
});

test('filters, paging, keyboard dialogs and conservative batch actions', async ({ page }) => {
  await analyzed(page);
  await page.getByLabel('Résultats par page').selectOption('25');
  await page.getByRole('button', { name: 'Page suivante', exact: true }).click();
  await expect(page.getByTestId('finding').first()).toContainText('Marchés fictifs!27:');
  await page.getByLabel('Rechercher dans les résultats').fill('phone_multiple');
  await expect(page.getByTestId('finding').first()).toContainText('Plusieurs numéros');
  await page.getByTestId('finding').first().getByRole('button', { name: 'Examiner et décider' }).click();
  await expect(page.getByRole('dialog')).toBeVisible(); await page.keyboard.press('Escape'); await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.getByLabel('Sélectionner les cellules de cette page').check();
  await page.getByRole('button', { name: 'Conserver', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Ajouter au lot', exact: true }).click();
  await page.getByRole('button', { name: 'Vérifier le lot', exact: true }).click();
  await expect(page.getByTestId('finding').first()).toContainText('Plusieurs numéros');
});

test('Pages hash refresh is nonblank, and no external fonts or services are requested', async ({ page }, info) => {
  const external: string[] = [];
  page.on('request', request => { if (/^https?:/.test(request.url()) && !request.url().startsWith('http://127.0.0.1:4173/')) external.push(request.url()); });
  await page.goto('/#/projet/missing/association');
  await expect(page.getByRole('heading', { name: 'Aucun projet ouvert pour cette adresse' })).toBeVisible();
  await page.reload(); await expect(page.getByRole('heading', { name: 'Aucun projet ouvert pour cette adresse' })).toBeVisible();
  await page.getByRole('button', { name: 'Retour à l’accueil', exact: true }).click();
  await expect(page.getByTestId('try-demo')).toBeVisible();
  expect(['normal', '0px']).toContain(await page.locator('body').evaluate(element => getComputedStyle(element).letterSpacing));
  expect(external).toEqual([]); await noHorizontalOverflow(page);
  await page.screenshot({ path: info.outputPath('home.png'), fullPage: true });
});

test('HTML startup remains visible when all script downloads fail', async ({ page }, info) => {
  await page.route('**/*.js', route => route.abort());
  await page.goto('/');
  await expect(page.getByTestId('startup-fallback')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'FDX Atelier', exact: true })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Recharger l’atelier' })).toBeVisible();
  await page.screenshot({ path: info.outputPath('startup-network-failure.png'), fullPage: true });
});

test('external boot and bundled fonts work with desktop-style CSP', async ({ page }) => {
  await page.route('http://127.0.0.1:4173/', async route => {
    const response = await route.fetch();
    await route.fulfill({ response, headers: { ...response.headers(), 'content-security-policy': "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' data: blob:" } });
  });
  await page.goto('/'); await expect(page.getByTestId('try-demo')).toBeVisible();
  await page.getByTestId('try-demo').click(); await expect(page.getByRole('heading', { name: 'Importer les sources', exact: true })).toBeVisible();
});
