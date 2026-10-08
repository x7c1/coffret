import { useEffect, type CSSProperties } from 'react';

import {
  choiceLabels,
  existsLine,
  folderLine,
  looseLine,
  questionOf,
  type Choice,
  type DropSummary,
} from './packChoice';
import { COLOR } from './theme';

/**
 * The question a drop holding a folder is asked before anything is sent: each
 * dropped folder with how many files it holds and how much they come to, and
 * three answers — a Pack, the files one by one, or nothing at all.
 *
 * A folder the Library already has where the drop lands is said to exist, and
 * the same answers are offered all the same. Adding goes as it always does: a
 * file whose path holds an Entry inside a Pack is refused on its own, and one
 * whose path holds a one-file Entry replaces that file. A Pack chosen here packs
 * only the files this drop carries — a replaced one among them — never the
 * other files already in that folder (spec: PK-17).
 *
 * Escape and a click outside it are Cancel, as they close any dialog — and
 * Cancel costs nothing, because nothing has been sent.
 */
export function PackConfirm({
  summary,
  onChoose,
}: {
  summary: DropSummary;
  onChoose: (choice: Choice) => void;
}) {
  useEffect(() => {
    const closing = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onChoose('cancel');
      }
    };
    window.addEventListener('keydown', closing);
    return () => window.removeEventListener('keydown', closing);
  }, [onChoose]);

  const labels = choiceLabels(summary);
  const question = questionOf(summary);
  return (
    <div
      role="presentation"
      onClick={() => onChoose('cancel')}
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
        aria-label={question}
        onClick={(event) => event.stopPropagation()}
        style={{
          width: 'min(520px, calc(100vw - 32px))',
          maxHeight: 'calc(100vh - 64px)',
          overflow: 'auto',
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
        <p style={{ margin: 0 }}>{question}</p>
        <ul style={{ margin: 0, padding: 0, listStyle: 'none' }}>
          {summary.folders.map((folder) => (
            <li key={folder.name} style={{ padding: '2px 0' }}>
              {folderLine(folder)}
              {folder.exists && (
                <span style={{ display: 'block', color: COLOR.warn, paddingLeft: 22 }}>
                  {existsLine(folder)}
                </span>
              )}
            </li>
          ))}
        </ul>
        {summary.loose > 0 && (
          <p style={{ margin: 0, color: COLOR.dim }}>{looseLine(summary)}</p>
        )}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          <button type="button" autoFocus onClick={() => onChoose('pack')} style={CHOICE}>
            {labels.pack}
          </button>
          <button type="button" onClick={() => onChoose('one_by_one')} style={CHOICE}>
            {labels.one_by_one}
          </button>
          <button type="button" onClick={() => onChoose('cancel')} style={CHOICE}>
            {labels.cancel}
          </button>
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
