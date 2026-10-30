import { useState, type FormEvent } from 'react';

import { placeOf } from './destination';
import { BUTTON, Dialog, FIELD, LINE, Place, PRIMARY } from './Dialog';
import { COLOR } from './theme';

/**
 * The question "New folder…" asks: `New folder in <path>`, and one Name field.
 *
 * What it creates is a path and nothing else — nothing is sent and nothing is
 * made on disk until files are added to it, and a reload forgets it — so the
 * dialog says so rather than leaving a person to find out.
 *
 * A name the Library would refuse — empty, holding a `/`, `.` or `..`, or one
 * the folder already holds (see [`prepare`](./prospective)) — is refused here,
 * in the dialog, and nothing is created: the reason stands under the field and
 * the name can be typed again. Escape, a click outside it and Cancel close it
 * having created nothing.
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
    <Dialog
      title={`New folder in ${placeOf(parent)}`}
      heading={
        <>
          New folder in <Place path={parent} />
        </>
      }
      onCancel={onClose}
      onSubmit={submit}
      actions={
        <>
          <button type="button" onClick={onClose} style={BUTTON}>
            Cancel
          </button>
          <button type="submit" disabled={asking} style={PRIMARY}>
            Create
          </button>
        </>
      }
    >
      <label style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        <span style={{ color: COLOR.dim, fontSize: 12 }}>Name</span>
        <input
          autoFocus
          aria-label="folder name"
          aria-invalid={refused !== null}
          value={typed}
          onChange={(event) => {
            setTyped(event.target.value);
            setRefused(null);
          }}
          style={{ ...FIELD, borderColor: refused === null ? COLOR.border : COLOR.refused }}
        />
      </label>
      {refused !== null && (
        <p role="alert" style={{ ...LINE, marginTop: -6, color: COLOR.refused, fontSize: 12 }}>
          {refused}
        </p>
      )}
      <p style={{ ...LINE, color: COLOR.dim }}>{CREATES_NOTHING_YET}</p>
    </Dialog>
  );
}

/** What the dialog says it does, and does not do. */
export const CREATES_NOTHING_YET = 'Nothing is created until you add files to it.';
