import { Component, type ReactNode } from 'react';

export class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state: { error: Error | null } = { error: null };
  static getDerivedStateFromError(error: unknown) { return { error: error instanceof Error ? error : new Error(String(error)) }; }
  render() {
    if (!this.state.error) return this.props.children;
    return <main className="fatal-error" role="alert"><div className="brand-mark">F</div><h1>FDX Atelier</h1><h2>L’affichage a rencontré une erreur.</h2><p>Les fichiers sources restent intacts. Réessayez l’affichage avant de recharger pour préserver la session en cours.</p><details><summary>Détails techniques</summary><pre>{this.state.error.message}</pre></details><div className="actions"><button className="btn btn-primary" onClick={() => this.setState({ error: null })}>Réessayer l’affichage</button><a className="btn" href="">Recharger l’atelier</a></div></main>;
  }
}
