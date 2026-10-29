import { useCallback } from 'react';

import { NOT_UNDOABLE, startsAfterLine, type DeleteQuestion } from './deleteEntries';
import { BUTTON, DANGER, Dialog, LINE, Note } from './Dialog';
import { COLOR } from './theme';
import type { WaitsFor } from './turns';

/**
 * The question "Delete…" asks before anything is armed: `Delete “albums”?`
 * (or `Delete 3 items?`), what leaves the Library, and that it cannot be
 * undone from the explorer — with two answers, Cancel or Delete.
 *
 * Where Packs that keep other files must be rebuilt, how many and what that
 * reads and writes stand out in a box of their own: rebuilding reads each such
 * Pack whole and uploads its replacement, the one part of a deletion that
 * costs time and transfer (spec: PK-10). The lines for Packs the deletion
 * would be refused for, named files the Library does not hold, and a deletion
 * that waits for another are each drawn only where they apply.
 *
 * Where nothing would be removed it says so and offers only Close. Where a
 * sync or a freeze is under way it says the deletion starts after it, since
 * the three take turns at this device's pending work.
 *
 * Escape and a click outside it are Cancel, as they close any dialog — and
 * Cancel costs nothing, because nothing has been armed. Cancel has the focus
 * rather than Delete: an Enter pressed out of habit must not delete anything.
 */
export function DeleteConfirm({
  question,
  startsAfter = null,
  onChoose,
}: {
  question: DeleteQuestion;
  /**
   * The sync or freeze holding the turn as the question is shown, which a
   * deletion confirmed now starts after — said here, so the person learns it before
   * confirming rather than from the progress line after.
   */
  startsAfter?: WaitsFor;
  onChoose: (remove: boolean) => void;
}) {
  const cancel = useCallback(() => onChoose(false), [onChoose]);
  return (
    <Dialog
      title={question.title}
      onCancel={cancel}
      actions={
        question.deletes ? (
          <>
            <button type="button" autoFocus onClick={cancel} style={BUTTON}>
              Cancel
            </button>
            <button type="button" onClick={() => onChoose(true)} style={DANGER}>
              Delete
            </button>
          </>
        ) : (
          <button type="button" autoFocus onClick={cancel} style={BUTTON}>
            Close
          </button>
        )
      }
    >
      <p style={LINE}>{question.removes}</p>
      {question.rebuilds !== null && (
        <Note tone="warn">
          <span>{question.rebuilds.line}</span>
          <span style={{ fontSize: 12 }}>{question.rebuilds.cost}</span>
        </Note>
      )}
      {question.refused.map((line) => (
        <p key={line} style={{ ...LINE, color: COLOR.warn }}>
          {line}
        </p>
      ))}
      {question.missing !== null && <p style={{ ...LINE, color: COLOR.dim }}>{question.missing}</p>}
      {question.waits !== null && <p style={{ ...LINE, color: COLOR.warn }}>{question.waits}</p>}
      {question.deletes && startsAfter !== null && (
        <p style={{ ...LINE, color: COLOR.warn }}>{startsAfterLine(startsAfter)}</p>
      )}
      {question.deletes && <p style={{ ...LINE, color: COLOR.dim }}>{NOT_UNDOABLE}</p>}
    </Dialog>
  );
}
