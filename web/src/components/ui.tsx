import { cloneElement, useId, useLayoutEffect, useRef, useState, type ButtonHTMLAttributes, type ReactElement, type ReactNode } from 'react';
import { ChevronLeft, ChevronRight, Info, X, type LucideIcon } from 'lucide-react';

export function Button({ icon: Icon, children, primary, className = '', ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { icon?: LucideIcon; primary?: boolean }) {
  return <button type="button" {...props} className={`btn ${primary ? 'btn-primary' : ''} ${className}`}>{Icon && <Icon size={16} aria-hidden="true" />}{children}</button>;
}
export function IconButton({ icon: Icon, label, tooltip = true, className = '', onClick, onPointerEnter, onFocus, onKeyDown, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { icon: LucideIcon; label: string; tooltip?: boolean }) {
  const [dismissed, setDismissed] = useState(false);
  return <span className="tooltip-wrap" data-tooltip-dismissed={dismissed || undefined}><button type="button" {...props} className={`btn icon-btn ${className}`} aria-label={label} title={undefined}
    onPointerEnter={event => { if (event.pointerType !== 'touch') setDismissed(false); onPointerEnter?.(event); }}
    onFocus={event => { if (event.currentTarget.matches(':focus-visible') && event.relatedTarget instanceof Element && !event.relatedTarget.closest('dialog')) setDismissed(false); onFocus?.(event); }}
    onKeyDown={event => { if (event.key === 'Escape') setDismissed(true); onKeyDown?.(event); }}
    onClick={event => { setDismissed(true); onClick?.(event); }}
  ><Icon size={17} aria-hidden="true" /></button>{tooltip && <span className="tooltip" role="tooltip">{label}</span>}</span>;
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
  useLayoutEffect(() => {
    const dialog = ref.current!;
    const previous = document.activeElement as HTMLElement | null;
    const updateZoom = () => {
      let zoom = 1;
      // Viewport units do not shrink with CSS zoom; include each ancestor's local zoom.
      for (let element: HTMLElement | null = dialog; element; element = element.parentElement) {
        const value = getComputedStyle(element).getPropertyValue('zoom').trim();
        const scale = Number.parseFloat(value) / (value.endsWith('%') ? 100 : 1);
        if (Number.isFinite(scale) && scale > 0) zoom *= scale;
      }
      dialog.style.setProperty('--modal-effective-zoom', String(zoom));
    };
    updateZoom();
    dialog.showModal();
    window.addEventListener('resize', updateZoom);
    return () => { window.removeEventListener('resize', updateZoom); dialog.close(); if (previous?.isConnected) previous.focus({ preventScroll: true }); };
  }, []);
  return <dialog ref={ref} className={`modal ${wide ? 'modal-wide' : ''}`} aria-labelledby={titleId} onCancel={e => { e.preventDefault(); close.current(); }}>
    <header className="modal-head"><h2 id={titleId}>{title}</h2><IconButton icon={X} label="Fermer la fenêtre" tooltip={false} onClick={onClose} /></header>{children}
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
