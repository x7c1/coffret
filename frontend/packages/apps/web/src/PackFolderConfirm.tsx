import { useEffect, type CSSProperties } from 'react';

import type { PackQuestion } from './packFolder';
import { COLOR } from './theme';

/**
 * The question "Pack this folder…" asks before anything is armed: the folder
 * with how many files a freeze of it would pack and how much they come to,
 * what it would leave as it is, and two answers — Pack or Cancel.
 *
 * Where nothing would be packed it says so and why — the folder being packed
 * already among the reasons — and offers only Close: there is nothing for
 * Pack to do.
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
        aria-label={`pack ${question.folder}`}
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
        <p style={{ margin: 0 }}>{question.line}</p>
        {question.leftOut !== null && (
          <p style={{ margin: 0, color: COLOR.dim }}>{question.leftOut}</p>
        )}
        {question.waits !== null && (
          <p style={{ margin: 0, color: COLOR.warn }}>{question.waits}</p>
        )}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          {question.packs ? (
            <>
              <button type="button" autoFocus onClick={() => onChoose(true)} style={CHOICE}>
                Pack
              </button>
              <button type="button" onClick={() => onChoose(false)} style={CHOICE}>
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
