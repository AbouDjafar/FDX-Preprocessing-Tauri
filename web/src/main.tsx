import { createRoot } from 'react-dom/client';
import '@fontsource/dm-sans/latin-400.css';
import '@fontsource/dm-sans/latin-500.css';
import '@fontsource/dm-sans/latin-600.css';
import '@fontsource/dm-sans/latin-700.css';
import '@fontsource/fraunces/latin-500.css';
import '@fontsource/fraunces/latin-600.css';
import '@fontsource/dm-mono/latin-400.css';
import './index.css';
import App from './App';
import { ErrorBoundary } from './components/error-boundary';
import { WorkspaceProvider } from './workspace';

createRoot(document.getElementById('root')!).render(<ErrorBoundary><WorkspaceProvider><App /></WorkspaceProvider></ErrorBoundary>);
