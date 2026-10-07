import { useEffect, useMemo, useState, type CSSProperties } from 'react';

import { browseFolders, mapFolder, type Mapped } from '@coffret/api';

import {
  browseTo,
  descend,
  mapHere,
  MAP_HERE,
  pickerFor,
  pickerTitle,
  toParent,
  typedOver,
  type Asking,
  type Picker,
} from './mapping';
import { COLOR } from './theme';

/**
 * The small picker a banner's *map* button opens: the folder being shown, the
 * folders inside it to go into, the one above it, a field holding its path
 * that can be typed into, and one button that maps whatever the field holds.
 *
 * It starts at the home directory. A refusal is said in it and it stays open;
 * a mapping recorded closes it, and the screen reloads the folder it was
 * opened over (see [`mapping`](./mapping)).
 */
export function MapPicker({
  prefix,
  onMapped,
  onClose,
}: {
  /** The top-level folder of the Library being mapped, and `null` for its root. */
  prefix: string | null;
  onMapped: (answer: Mapped) => void;
  onClose: () => void;
}) {
  const [picker, setPicker] = useState<Picker>(() => pickerFor(prefix));
  const asking: Asking = useMemo(
    () => ({
      browse: (path) => browseFolders(path),
      map: (localRoot, mapped) => mapFolder(localRoot, mapped),
      update: setPicker,
      mapped: onMapped,
    }),
    [onMapped],
  );

  // The home directory, once, as the picker opens: `asking` changes only with
  // the handler, and the first folder is asked for once whatever happens to it.
  useEffect(() => {
    void browseTo(null, asking);
  }, []);

  // Escape closes it, as it closes any dialog.
  useEffect(() => {
    const closing = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
      }
    };
    window.addEventListener('keydown', closing);
    return () => window.removeEventListener('keydown', closing);
  }, [onClose]);

  const parent = picker.browsed?.parent ?? null;
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
      <div
        role="dialog"
        aria-label={pickerTitle(prefix)}
        onClick={(event) => event.stopPropagation()}
        style={{
          width: 'min(560px, calc(100vw - 32px))',
          maxHeight: 'calc(100vh - 64px)',
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
        <p style={{ margin: 0 }}>{pickerTitle(prefix)}</p>
        <form
          style={{ display: 'flex', gap: 8 }}
          // Enter in the field lists what was typed, so a path can be typed and
          // then browsed from as well as mapped outright.
          onSubmit={(event) => {
            event.preventDefault();
            void browseTo(picker.typed.trim() === '' ? null : picker.typed.trim(), asking);
          }}
        >
          <input
            aria-label="folder on this device"
            value={picker.typed}
            onChange={(event) => setPicker((held) => typedOver(held, event.target.value))}
            spellCheck={false}
            style={{ ...CONTROL, flex: 1, minWidth: 0 }}
          />
        </form>
        <div
          style={{
            flex: 1,
            minHeight: 120,
            overflow: 'auto',
            border: `1px solid ${COLOR.rowRule}`,
            borderRadius: 4,
          }}
        >
          {parent !== null && (
            <button
              type="button"
              disabled={picker.busy}
              onClick={() => void toParent(picker, asking)}
              style={ROW}
            >
              ↑ {parent}
            </button>
          )}
          {picker.browsed?.folders.map((folder) => (
            <button
              key={folder.path}
              type="button"
              disabled={picker.busy}
              onClick={() => void descend(folder, asking)}
              style={ROW}
            >
              ▸ {folder.name}
            </button>
          ))}
          {picker.browsed !== null && picker.browsed.folders.length === 0 && (
            <p style={{ margin: 0, padding: '5px 10px', color: COLOR.dim }}>no folders in here</p>
          )}
        </div>
        {picker.refused !== null && (
          <p style={{ margin: 0, color: COLOR.refused }}>{picker.refused}</p>
        )}
        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
          <button type="button" onClick={onClose} style={CONTROL}>
            cancel
          </button>
          <button
            type="button"
            disabled={picker.busy}
            onClick={() => void mapHere(picker, asking)}
            style={CONTROL}
          >
            {MAP_HERE}
          </button>
        </div>
      </div>
    </div>
  );
}

const CONTROL: CSSProperties = {
  border: `1px solid ${COLOR.border}`,
  background: COLOR.panel,
  color: COLOR.text,
  font: 'inherit',
  padding: '3px 10px',
  borderRadius: 4,
};

const ROW: CSSProperties = {
  display: 'block',
  width: '100%',
  textAlign: 'left',
  border: 'none',
  borderBottom: `1px solid ${COLOR.rowRule}`,
  background: 'transparent',
  color: COLOR.text,
  font: 'inherit',
  padding: '5px 10px',
  cursor: 'pointer',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
};
