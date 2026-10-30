import { expect, test, type Page } from '@playwright/test';

async function demo(page: Page) {
  await page.goto('/');
  await page.getByTestId('try-demo').click();
  await expect(page.getByRole('heading', { name: 'Importer les sources', exact: true })).toBeVisible();
}

test('preview is opt-in, can be hidden and does not gate importing', async ({ page }, info) => {
  await demo(page);
  const preview = page.getByRole('table', { name: 'Aperçu de la feuille sélectionnée, avec positions Excel', exact: true });
  const show = page.getByRole('button', { name: 'Voir l’aperçu', exact: true });
  const commit = page.getByRole('button', { name: 'Importer et associer', exact: true });
  await expect(preview).toHaveCount(0);
  await expect(show).toHaveAttribute('aria-expanded', 'false');
  await expect(commit).toBeEnabled();
  await show.click();
  await expect(preview).toBeVisible();
  await expect(preview).toContainText('DEMO/0001');
  const hide = page.getByRole('button', { name: 'Masquer l’aperçu', exact: true });
  await expect(hide).toHaveAttribute('aria-expanded', 'true');
  expect(await hide.getAttribute('aria-controls')).toBeTruthy();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)).toBe(true);
  await page.screenshot({ path: info.outputPath('import-preview.png'), fullPage: true });
  await hide.click();
  await expect(preview).toHaveCount(0);
  await expect(show).toHaveAttribute('aria-expanded', 'false');
  await expect(commit).toBeEnabled();
});

test('sheet, header, refresh and source selection all leave preview hidden', async ({ page }) => {
  await demo(page);
  const show = page.getByRole('button', { name: 'Voir l’aperçu', exact: true });
  const preview = page.getByRole('table', { name: 'Aperçu de la feuille sélectionnée, avec positions Excel', exact: true });
  await show.click();
  await page.getByLabel('Feuille de calcul', { exact: true }).selectOption('Annexe fictive');
  await expect(preview).toHaveCount(0);
  await expect(show).toHaveAttribute('aria-expanded', 'false');
  await show.click();
  await expect(preview).toContainText('Annexe conservée');
  await page.getByLabel('Ligne d’en-tête', { exact: true }).fill('2');
  await expect(preview).toHaveCount(0);
  await expect(show).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Importer et associer', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Actualiser l’aperçu', exact: true }).click();
  await expect(show).toBeEnabled();
  await expect(preview).toHaveCount(0);
  await show.click();
  await page.getByRole('button', { name: 'Actualiser l’aperçu', exact: true }).click();
  await expect(show).toBeEnabled();
  await expect(preview).toHaveCount(0);
  await show.click();
  await page.getByRole('button', { name: 'Choisir l’exemple', exact: true }).click();
  await expect(show).toBeEnabled();
  await expect(show).toHaveAttribute('aria-expanded', 'false');
  await expect(preview).toHaveCount(0);
});

test('tooltips dismiss on Escape and activation, and stay absent behind dialogs', async ({ page }) => {
  await demo(page);
  await page.getByTestId('stage-association').click();
  const edit = page.getByRole('button', { name: 'Modifier MONTANT', exact: true });
  const tooltip = page.getByRole('tooltip').filter({ hasText: /^Modifier MONTANT$/ });
  await edit.focus();
  await page.keyboard.press('Tab');
  await page.keyboard.press('Shift+Tab');
  await expect(edit).toBeFocused();
  await expect(tooltip).toBeVisible();
  await expect(edit).not.toHaveAttribute('title');
  await page.keyboard.press('Escape');
  await expect(tooltip).toBeHidden();
  await page.keyboard.press('Tab');
  await page.keyboard.press('Shift+Tab');
  await expect(tooltip).toBeVisible();
  await edit.press('Enter');
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  await expect(page.locator('.tooltip:visible')).toHaveCount(0);
  const close = dialog.getByRole('button', { name: 'Fermer la fenêtre', exact: true });
  await close.focus();
  await expect(close).toBeFocused();
  await expect(close).not.toHaveAttribute('title');
  await expect(page.locator('.tooltip:visible')).toHaveCount(0);
  await close.press('Enter');
  await expect(dialog).toHaveCount(0);
  await expect(edit).toBeFocused();
  await expect(page.locator('.tooltip:visible')).toHaveCount(0);
  await page.keyboard.press('Tab');
  await page.keyboard.press('Shift+Tab');
  await expect(tooltip).toBeVisible();
});

test('refresh turns once and press feedback never changes control geometry', async ({ page }) => {
  await page.goto('/');
  const refresh = page.getByRole('button', { name: 'Actualiser les projets récents', exact: true });
  const icon = refresh.locator('svg');
  for (const reducedMotion of ['no-preference', 'reduce'] as const) {
    await page.emulateMedia({ reducedMotion });
    await page.mouse.move(0, 0);
    await refresh.scrollIntoViewIfNeeded();
    const before = await refresh.boundingBox();
    await refresh.hover();
    await expect(icon).toHaveCSS('animation-name', reducedMotion === 'reduce' ? 'none' : 'rotate');
    expect(await refresh.boundingBox()).toEqual(before);
    await page.mouse.down();
    await expect.poll(() => icon.evaluate(element => getComputedStyle(element).scale)).toBe(reducedMotion === 'reduce' ? 'none' : '0.9');
    expect(await refresh.boundingBox()).toEqual(before);
    if (reducedMotion === 'reduce') {
      await expect(icon).toHaveCSS('transform', 'none');
      await expect(icon).toHaveCSS('transition-duration', '0s');
    } else {
      const transforms = await page.evaluate(() => Array.from(document.styleSheets)
        .flatMap(sheet => Array.from(sheet.cssRules))
        .filter((rule): rule is CSSKeyframesRule => rule instanceof CSSKeyframesRule && rule.name === 'rotate')
        .flatMap(rule => Array.from(rule.cssRules)
          .filter((frame): frame is CSSKeyframeRule => frame instanceof CSSKeyframeRule)
          .map(frame => frame.style.transform)));
      expect(transforms).toContain('rotate(360deg)');
    }
    await page.mouse.up();
    await expect(refresh).toBeEnabled();
  }
});
