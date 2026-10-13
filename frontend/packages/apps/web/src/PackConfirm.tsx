import { useEffect, useMemo, useState, type CSSProperties } from 'react';

import type { Added } from '@coffret/api';

import {
  choiceLabels,
  existsLine,
  folderLine,
  looseLine,
  questionOf,
  type Choice,
  type DropSummary,
} from './packChoice';
import { recall, remember } from './remembered';
import {
  elided,
  groupHeading,
  hasHidden,
  leftWholeLine,
  renameLine,
  renumber,
  unchangedLine,
  type Group,
  type Numbering,
} from './renumber';
import { COLOR } from './theme';

/**
 * Where the two boxes are remembered. A person scanning books drops one after
 * another and wants the same answer each time, so ticking a box is kept for the
 * next drop rather than asked again.
 */
const REMEMBERED = {
  pages: 'coffret.drop.number-pages',
  volumes: 'coffret.drop.number-volumes',
} as const;

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
 * Below the answers, two boxes offer to number the pages and the volume
 * folders of what was dropped, off unless the last drop ticked them. Ticking
 * one shows what it would rename before anything is chosen, because a rename
 * is only worth having where it reads the names the way the person meant —
 * and the answer then carries the boxes beside it, whichever it is.
 *
 * Escape and a click outside it are Cancel, as they close any dialog — and
 * Cancel costs nothing, because nothing has been sent.
 */
export function PackConfirm({
  summary,
  files,
  onChoose,
}: {
  summary: DropSummary;
  /** The drop's files as dropped, which the preview is worked out from. */
  files: readonly Added[];
  onChoose: (choice: Choice, numbering: Numbering) => void;
}) {
  const [numbering, setNumbering] = useState<Numbering>(() => ({
    pages: recall(REMEMBERED.pages),
    volumes: recall(REMEMBERED.volumes),
  }));
  const tick = (which: keyof Numbering, on: boolean) => {
    remember(REMEMBERED[which], on);
    setNumbering((held) => ({ ...held, [which]: on }));
  };
  const answer = (choice: Choice) => onChoose(choice, numbering);

  useEffect(() => {
    const closing = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onChoose('cancel', numbering);
      }
    };
    window.addEventListener('keydown', closing);
    return () => window.removeEventListener('keydown', closing);
  }, [onChoose, numbering]);

  const groups = useMemo(() => renumber(files, numbering).groups, [files, numbering]);

  const labels = choiceLabels(summary);
  const question = questionOf(summary);
  return (
    <div
      role="presentation"
      onClick={() => answer('cancel')}
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
          <button type="button" autoFocus onClick={() => answer('pack')} style={CHOICE}>
            {labels.pack}
          </button>
          <button type="button" onClick={() => answer('one_by_one')} style={CHOICE}>
            {labels.one_by_one}
          </button>
          <button type="button" onClick={() => answer('cancel')} style={CHOICE}>
            {labels.cancel}
          </button>
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
          <label style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
            <input
              type="checkbox"
              checked={numbering.pages}
              onChange={(event) => tick('pages', event.target.checked)}
            />
            Number the pages
          </label>
          <label style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
            <input
              type="checkbox"
              checked={numbering.volumes}
              onChange={(event) => tick('volumes', event.target.checked)}
            />
            Number the volume folders
          </label>
        </div>
        {(numbering.pages || numbering.volumes) && <Preview groups={groups} />}
      </div>
    </div>
  );
}

/**
 * What the ticked boxes would rename, group by group: both ends of each run of
 * renames and the names left as they are, with a way to see all of it.
 */
function Preview({ groups }: { groups: readonly Group[] }) {
  const [all, setAll] = useState(false);
  if (groups.length === 0) {
    return <p style={{ margin: 0, color: COLOR.dim }}>nothing in this drop is renamed</p>;
  }
  return (
    <div
      aria-label="what is renamed"
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
        padding: 8,
        border: `1px solid ${COLOR.border}`,
        borderRadius: 4,
        overflowWrap: 'anywhere',
      }}
    >
      {groups.map((group) => {
        const renamed = elided(group.renamed, all);
        const unchanged = elided(group.unchanged, all);
        return (
          <div key={`${group.kind}:${group.folder}`}>
            <p style={{ margin: 0, color: COLOR.dim }}>{groupHeading(group)}</p>
            {group.leftWhole !== null && (
              <p style={{ margin: 0, color: COLOR.warn }}>{leftWholeLine(group.leftWhole)}</p>
            )}
            <ul style={{ margin: 0, paddingLeft: 16 }}>
              {renamed.head.map((rename) => (
                <li key={rename.from}>{renameLine(rename)}</li>
              ))}
              {renamed.hidden > 0 && <li style={{ color: COLOR.dim }}>… {renamed.hidden} more …</li>}
              {renamed.tail.map((rename) => (
                <li key={rename.from}>{renameLine(rename)}</li>
              ))}
              {unchanged.head.map((kept) => (
                <li key={kept.from} style={{ color: COLOR.dim }}>
                  {unchangedLine(kept)}
                </li>
              ))}
              {unchanged.hidden > 0 && (
                <li style={{ color: COLOR.dim }}>… {unchanged.hidden} more unchanged …</li>
              )}
              {unchanged.tail.map((kept) => (
                <li key={kept.from} style={{ color: COLOR.dim }}>
                  {unchangedLine(kept)}
                </li>
              ))}
            </ul>
          </div>
        );
      })}
      {(all || hasHidden(groups)) && (
        <button type="button" onClick={() => setAll(!all)} style={{ ...CHOICE, alignSelf: 'start' }}>
          {all ? 'Show less' : 'Show all'}
        </button>
      )}
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
