import { cloneElement, useEffect, useId, useRef, type ButtonHTMLAttributes, type ReactElement, type ReactNode } from 'react';
import { ChevronLeft, ChevronRight, Info, X, type LucideIcon } from 'lucide-react';

export function Button({ icon: Icon, children, primary, className = '', ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { icon?: LucideIcon; primary?: boolean }) {
  return <button type="button" {...props} className={`btn ${primary ? 'btn-primary' : ''} ${className}`}>{Icon && <Icon size={16} aria-hidden="true" />}{children}</button>;
}
export function IconButton({ icon: Icon, label, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { icon: LucideIcon; label: string }) {
  return <span className="tooltip-wrap"><button type="button" {...props} className={`btn icon-btn ${props.className || ''}`} aria-label={label} title={label}><Icon size={17} aria-hidden="true" /></button><span className="tooltip" role="tooltip">{label}</span></span>;
}
export function Field({ label, children, hint, className = '' }: { label: string; children: ReactElement<{ id?: string; 'aria-describedby'?: string }>; hint?: string; className?: string }) {
  const id = useId();
  const controlId = children.props.id || id;
  const hintId = `${id}-hint`;
  const describedBy = [children.props['aria-describedby'], hint ? hintId : undefined].filter(Boolean).join(' ') || undefined;
  return <div className={`field ${className}`}><label className="field-label" htmlFor={controlId}>{label}</label>{cloneElement(children, { id: controlId, 'aria-describedby': describedBy })}{hint && <span id={hintId} className="field-hint">{hint}</span>}</div>;
}
export function Toggle({ label, checked, onChange, disabled, hint }: { label: string; checked: boolean; onChange: (checked: boolean) => void; disabled?: boolean; hint?: string }) {
  const id = useId();
  const hintId = `${id}-hint`;
  return <div className="switch-row"><span><label htmlFor={id}><strong>{label}</strong></label>{hint && <span id={hintId} className="field-hint">{hint}</span>}</span><input id={id} aria-describedby={hint ? hintId : undefined} className="toggle" type="checkbox" checked={checked} disabled={disabled} onChange={e => onChange(e.target.checked)} /></div>;
}
export function Notice({ children, warm = false }: { children: ReactNode; warm?: boolean }) {
  return <div className={`notice ${warm ? 'warm' : ''}`}><Info size={17} aria-hidden="true" /><div>{children}</div></div>;
}
export function Section({ title, children, actions }: { title: string; children: ReactNode; actions?: ReactNode }) {
  return <section className="section"><header className="section-head"><h2>{title}</h2>{actions}</header>{children}</section>;
}
export function Modal({ title, children, onClose, wide = false }: { title: string; children: ReactNode; onClose: () => void; wide?: boolean }) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const close = useRef(onClose); close.current = onClose;
  useEffect(() => {
    const dialog = ref.current!;
    const previous = document.activeElement as HTMLElement | null;
    dialog.showModal();
    return () => { dialog.close(); if (previous?.isConnected) previous.focus(); };
  }, []);
  return <dialog ref={ref} className={`modal ${wide ? 'modal-wide' : ''}`} aria-labelledby={titleId} onCancel={e => { e.preventDefault(); close.current(); }}>
    <header className="modal-head"><h2 id={titleId}>{title}</h2><IconButton icon={X} label="Fermer la fenêtre" onClick={onClose} /></header>{children}
  </dialog>;
}
export function Pager({ offset, limit, total, onPage, onLimit, disabled }: { offset: number; limit: number; total: number; onPage: (offset: number) => void; onLimit?: (limit: number) => void; disabled?: boolean }) {
  return <div className="pager"><span className="mono" aria-live="polite">{total ? offset + 1 : 0}–{Math.min(offset + limit, total)} / {total.toLocaleString('fr-FR')}</span><div className="actions">
    {onLimit && <label className="inline-label">Par page<select aria-label="Résultats par page" value={limit} onChange={e => onLimit(Number(e.target.value))} disabled={disabled}>{[25, 50, 100, 250].map(n => <option key={n} value={n}>{n}</option>)}</select></label>}
    <IconButton icon={ChevronLeft} label="Page précédente" disabled={disabled || offset === 0} onClick={() => onPage(Math.max(0, offset - limit))} />
    <IconButton icon={ChevronRight} label="Page suivante" disabled={disabled || offset + limit >= total} onClick={() => onPage(offset + limit)} />
  </div></div>;
}
export function Empty({ title, children }: { title: string; children?: ReactNode }) { return <div className="empty-state"><h2>{title}</h2>{children}</div>; }
