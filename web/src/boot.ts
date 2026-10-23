const startup = document.getElementById('startup');
const message = document.getElementById('startup-message');
const slow = window.setTimeout(() => {
  if (startup?.isConnected && message) message.textContent = 'Le démarrage prend plus de temps que prévu. Vous pouvez patienter ou recharger l’atelier.';
}, 12000);

void import('./main').then(() => window.clearTimeout(slow)).catch((error: unknown) => {
  window.clearTimeout(slow);
  if (message) message.textContent = 'L’atelier n’a pas pu démarrer. Aucun fichier n’a été modifié.';
  const details = document.getElementById('startup-details');
  if (details) details.textContent = error instanceof Error ? error.message : String(error);
  document.getElementById('startup-error')?.removeAttribute('hidden');
});
