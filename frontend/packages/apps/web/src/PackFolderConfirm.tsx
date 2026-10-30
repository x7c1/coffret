import { useCallback } from 'react';

import { BUTTON, Dialog, LINE, Note, PRIMARY } from './Dialog';
import { PACK_EXPLAINED, type PackQuestion } from './packFolder';
import { COLOR } from './theme';

/**
 * The question "Pack this folder…" asks before anything is armed: `Pack
 * “BookX”?`, how many files a freeze of it would pack and how much they come
 * to, a "Left as they are" box listing what it would leave and why, and the
 * one line saying what a Pack is for — with two answers, Cancel or Pack.
 *
 * Where nothing would be packed it says so and why — the folder being packed
 * already among the reasons — and offers only Close: there is nothing for
 * Pack to do. Where another freeze is running it says this one waits for it.
 *
 * Escape and a click outside it are Cancel, as they close any dialog — and
 * Cancel costs nothing, because nothing has been armed.
 */
export function PackFolderConfirm({
  question,
  onChoose,
}: {
  question: PackQuestion;
  onChoose: (pack: boolean) => void;
}) {
  const cancel = useCallback(() => onChoose(false), [onChoose]);
  return (
    <Dialog
      title={question.title}
      onCancel={cancel}
      actions={
        question.packs ? (
          <>
            <button type="button" onClick={cancel} style={BUTTON}>
              Cancel
            </button>
            <button type="button" autoFocus onClick={() => onChoose(true)} style={PRIMARY}>
              Pack
            </button>
          </>
        ) : (
          <button type="button" autoFocus onClick={cancel} style={BUTTON}>
            Close
          </button>
        )
      }
    >
      <p style={LINE}>{question.line}</p>
      {question.leftOut.length > 0 && (
        <Note tone="plain">
          <span style={{ color: COLOR.dim, fontSize: 12 }}>Left as they are</span>
          <ul style={{ margin: 0, paddingLeft: 18 }}>
            {question.leftOut.map((group) => (
              <li key={group}>{group}</li>
            ))}
          </ul>
        </Note>
      )}
      {question.waits !== null && <p style={{ ...LINE, color: COLOR.warn }}>{question.waits}</p>}
      <p style={{ ...LINE, color: COLOR.dim }}>{PACK_EXPLAINED}</p>
    </Dialog>
  );
}
