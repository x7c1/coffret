import { useEffect, useState, type CSSProperties, type FormEvent } from 'react';

import { LIBRARY_ROOT } from './destination';
import { COLOR } from './theme';

/**
 * The question "New folder…" asks: one name, for a folder inside `parent`.
 *
 * What it prepares is a path and nothing else — nothing is sent and nothing is
 * made on disk until files are dropped into it, and a reload forgets it — so
 * the dialog says so under the field rather than leaving a person to find out.
 *
 * A name the Library would refuse is refused here, in the dialog, and nothing
 * is prepared: the reason stands under the field and the name can be typed
 * again. Escape, a click outside it and Cancel close it having prepared
 * nothing.
 */
export function NewFolderDialog({
  parent,
  onPrepare,
  onClose,
}: {
  /** The folder the new one goes in; the empty string is the Library root. */
  parent: string;
  /**
   * The name was given: prepare it, and answer with why not where it was
   * refused, or `null` where it was prepared.
   */
  onPrepare: (typed: string) => Promise<string | null>;
  onClose: () => void;
}) {
  const [typed, setTyped] = useState('');
  const [refused, setRefused] = useState<string | null>(null);
  const [asking, setAsking] = useState(false);
  useEffect(() => {
    const closing = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
      }
    };
    window.addEventListener('keydown', closing);
    return () => window.removeEventListener('keydown', closing);
  }, [onClose]);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (asking) {
      return;
    }
    setAsking(true);
    void onPrepare(typed).then((why) => {
      setAsking(false);
      setRefused(why);
    });
  };

  return (
    <div
      role="presentation"
      onClick={onClose}
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 10,
        background: 'rgba(0, 0, 0, 0.55)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}
    >
      <form
        role="dialog"
        aria-modal="true"
        aria-label="new folder"
        onClick={(event) => event.stopPropagation()}
        onSubmit={submit}
        style={{
          width: 'min(520px, calc(100vw - 32px))',
          display: 'flex',
          flexDirection: 'column',
          gap: 10,
          padding: 16,
          background: COLOR.panel,
          border: `1px solid ${COLOR.border}`,
          borderRadius: 6,
          color: COLOR.text,
          fontSize: 13,
        }}
      >
        <label style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          <span>
            A new folder in {parent === '' ? LIBRARY_ROOT : <code>{parent}/</code>}, called
          </span>
          <input
            autoFocus
            aria-label="folder name"
            value={typed}
            onChange={(event) => {
              setTyped(event.target.value);
              setRefused(null);
            }}
            style={FIELD}
          />
        </label>
        {refused !== null && (
          <p role="alert" style={{ margin: 0, color: COLOR.refused }}>
            {refused}
          </p>
        )}
        <p style={{ margin: 0, color: COLOR.dim }}>
          {PREPARES_ONLY}
        </p>
        <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
          <button type="button" onClick={onClose} style={CHOICE}>
            Cancel
          </button>
          <button type="submit" disabled={asking} style={CHOICE}>
            Prepare
          </button>
        </div>
      </form>
    </div>
  );
}

/** What the dialog says it does, and does not do. */
const PREPARES_ONLY =
  'Nothing is sent and nothing is made on disk: the folder is shown as not in the Library ' +
  'yet until files are dropped into it, and a reload forgets it.';

const FIELD: CSSProperties = {
  border: `1px solid ${COLOR.border}`,
  background: COLOR.ground,
  color: COLOR.text,
  font: 'inherit',
  padding: '4px 8px',
  borderRadius: 4,
};

const CHOICE: CSSProperties = {
  border: `1px solid ${COLOR.border}`,
  background: COLOR.panel,
  color: COLOR.text,
  font: 'inherit',
  padding: '5px 10px',
  borderRadius: 4,
  cursor: 'pointer',
};
