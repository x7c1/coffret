import { useEffect, type CSSProperties } from 'react';

import { NOT_UNDOABLE, startsAfterLine, type DeleteQuestion } from './deleteEntries';
import { COLOR } from './theme';
import type { WaitsFor } from './turns';

/**
 * The question "Delete…" asks before anything is armed: what leaves the
 * Library, what rebuilding the Packs that keep other files costs, anything the
 * deletion would be refused for and why, and that it cannot be undone from the
 * explorer — with two answers, Delete or Cancel.
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
   * The sync or freeze under way as the question is shown, which a deletion
   * confirmed now starts after — said here, so the person learns it before
   * confirming rather than from the progress line after.
   */
  startsAfter?: WaitsFor;
  onChoose: (remove: boolean) => void;
}) {
  useEffect(() => {
    const closing = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onChoose(false);
      }
    };
    window.addEventListener('keydown', closing);
    return () => window.removeEventListener('keydown', closing);
  }, [onChoose]);

  return (
    <div
      role="presentation"
      onClick={() => onChoose(false)}
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
        aria-modal="true"
        aria-label={question.title}
        onClick={(event) => event.stopPropagation()}
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
        <p style={{ margin: 0 }}>{question.title}</p>
        <p style={{ margin: 0 }}>{question.removes}</p>
        {question.rebuilds !== null && (
          <p style={{ margin: 0, color: COLOR.dim }}>{question.rebuilds}</p>
        )}
        {question.refused.map((line) => (
          <p key={line} style={{ margin: 0, color: COLOR.warn }}>
            {line}
          </p>
        ))}
        {question.missing !== null && (
          <p style={{ margin: 0, color: COLOR.dim }}>{question.missing}</p>
        )}
        {question.waits !== null && (
          <p style={{ margin: 0, color: COLOR.warn }}>{question.waits}</p>
        )}
        {question.deletes && startsAfter !== null && (
          <p style={{ margin: 0, color: COLOR.warn }}>{startsAfterLine(startsAfter)}</p>
        )}
        {question.deletes && <p style={{ margin: 0, color: COLOR.refused }}>{NOT_UNDOABLE}</p>}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          {question.deletes ? (
            <>
              <button type="button" onClick={() => onChoose(true)} style={CHOICE}>
                Delete
              </button>
              <button type="button" autoFocus onClick={() => onChoose(false)} style={CHOICE}>
                Cancel
              </button>
            </>
          ) : (
            <button type="button" autoFocus onClick={() => onChoose(false)} style={CHOICE}>
              Close
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

const CHOICE: CSSProperties = {
  border: `1px solid ${COLOR.border}`,
  background: COLOR.panel,
  color: COLOR.text,
  font: 'inherit',
  padding: '5px 10px',
  borderRadius: 4,
  textAlign: 'left',
  cursor: 'pointer',
};
