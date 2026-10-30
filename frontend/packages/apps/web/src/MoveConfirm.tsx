import { useCallback } from 'react';

import { BUTTON, Dialog, LINE, Note, PRIMARY } from './Dialog';
import type { MoveQuestion } from './moving';
import { COLOR } from './theme';

/**
 * The question a drag of rows onto a folder, and "Move to…", ask before
 * anything is written: `Move “Vol. 03” to “Series A”?` (or `Move 3 items to
 * …`), what the move does in the Library, and — set apart in a box of its own
 * — how many of this device's files cannot follow and leave it, with their
 * size (see [`moveQuestionOf`](./moving)).
 *
 * Where the server's preview refuses the move — a name the destination holds
 * already, an edited file, a file still on its way into the Library — it says
 * why and offers Cancel alone: nothing is overwritten or merged.
 *
 * Escape and a click outside it are Cancel, as they close any dialog. Cancel
 * has the focus rather than Move, so that an `Enter` pressed out of habit
 * moves nothing.
 */
export function MoveConfirm({
  question,
  onChoose,
}: {
  question: MoveQuestion;
  onChoose: (move: boolean) => void;
}) {
  const cancel = useCallback(() => onChoose(false), [onChoose]);
  return (
    <Dialog
      title={question.title}
      onCancel={cancel}
      actions={
        <>
          <button type="button" autoFocus onClick={cancel} style={BUTTON}>
            Cancel
          </button>
          {question.moves && (
            <button type="button" onClick={() => onChoose(true)} style={PRIMARY}>
              Move
            </button>
          )}
        </>
      }
    >
      {question.lines.map((line) => (
        <p key={line} style={LINE}>
          {line}
        </p>
      ))}
      {question.leaving !== null && (
        <Note tone="warn">
          <span>{question.leaving}</span>
        </Note>
      )}
      {question.refused.map((line) => (
        <p key={line} role="alert" style={{ ...LINE, color: COLOR.refused }}>
          {line}
        </p>
      ))}
    </Dialog>
  );
}
