import { useState, type FormEvent } from 'react';

import { BUTTON, Dialog, FIELD, LINE, Note, PRIMARY } from './Dialog';
import { COLOR } from './theme';

/** What a press of Rename came to, as the screen answers it. */
export type RenameAnswer =
  /** The name refused by the name rules, and why: said under the field, and the name can be typed again. */
  | { refused: string }
  /**
   * Refused for a reason that is not about the name — an edited file, an Entry
   * still uploading, the move itself refused: said in the dialog's body, the field left
   * as it is.
   */
  | { blocked: string }
  /** Files of this device would leave it: said in the dialog, and Rename again goes ahead. */
  | { leaving: string }
  /** Renamed, or nothing to rename: the dialog closes. */
  | null;

/**
 * The question "Rename…" asks, as "New folder…" asks its own: `Rename
 * “Vol. 03”`, and one Name field holding the current name, selected, so that
 * typing replaces it.
 *
 * A name that cannot be used — empty, holding `/`, `.` or `..`, one coffret
 * keeps for itself, or the name of another row in the same folder (see
 * [`renameDefect`](./moving)) — is
 * refused here with the reason under the field. A refusal from the server is
 * not about the name, and is said in the dialog's body instead, the
 * field not marked. Where renaming would leave some of this device's files
 * behind, the dialog says how many and their size first, as a move's
 * confirmation does, and Rename pressed again with the same name goes ahead.
 * Escape, a click outside it and Cancel close it having renamed nothing.
 */
export function RenameDialog({
  name,
  onRename,
  onClose,
}: {
  /** The name the row has now. */
  name: string;
  /**
   * The name was given: rename to it, `acknowledged` saying whether the files
   * that would leave this device were already said for this very name.
   */
  onRename: (typed: string, acknowledged: boolean) => Promise<RenameAnswer>;
  onClose: () => void;
}) {
  const [typed, setTyped] = useState(name);
  const [refused, setRefused] = useState<string | null>(null);
  const [blocked, setBlocked] = useState<string | null>(null);
  // What leaves this device, and the name it was said for.
  const [leaving, setLeaving] = useState<{ line: string; for: string } | null>(null);
  const [asking, setAsking] = useState(false);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (asking) {
      return;
    }
    setAsking(true);
    void onRename(typed, leaving !== null && leaving.for === typed).then((answer) => {
      setAsking(false);
      if (answer === null) {
        return;
      }
      setRefused('refused' in answer ? answer.refused : null);
      setBlocked('blocked' in answer ? answer.blocked : null);
      setLeaving('leaving' in answer ? { line: answer.leaving, for: typed } : null);
    });
  };

  return (
    <Dialog
      title={`Rename “${name}”`}
      onCancel={onClose}
      onSubmit={submit}
      actions={
        <>
          <button type="button" onClick={onClose} style={BUTTON}>
            Cancel
          </button>
          <button type="submit" disabled={asking} style={PRIMARY}>
            Rename
          </button>
        </>
      }
    >
      <label style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        <span style={{ color: COLOR.dim, fontSize: 12 }}>Name</span>
        <input
          autoFocus
          aria-label="new name"
          aria-invalid={refused !== null}
          value={typed}
          onFocus={(event) => event.currentTarget.select()}
          onChange={(event) => {
            setTyped(event.target.value);
            setRefused(null);
            setBlocked(null);
            setLeaving(null);
          }}
          style={{ ...FIELD, borderColor: refused === null ? COLOR.border : COLOR.refused }}
        />
      </label>
      {refused !== null && (
        <p role="alert" style={{ ...LINE, marginTop: -6, color: COLOR.refused, fontSize: 12 }}>
          {refused}
        </p>
      )}
      {blocked !== null && (
        <p role="alert" style={{ ...LINE, color: COLOR.refused }}>
          {blocked}
        </p>
      )}
      {leaving !== null && (
        <Note tone="warn">
          <span>{leaving.line}</span>
          <span style={{ fontSize: 12 }}>{RENAME_AGAIN}</span>
        </Note>
      )}
    </Dialog>
  );
}

/** Said under the files that would leave this device: what goes ahead with them. */
export const RENAME_AGAIN = 'Press Rename again to rename it all the same.';
