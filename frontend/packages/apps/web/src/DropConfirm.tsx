import { useEffect, useMemo, useState, type CSSProperties } from 'react';

import type { Added } from '@coffret/api';

import { newLine, pathLine, type Destination, type Level } from './destination';
import {
  ADD,
  CANCEL,
  choiceLabels,
  collisionsOf,
  existsLine,
  folderLine,
  looseLine,
  NOTHING_HELD,
  questionOf,
  refusedLine,
  replacesLine,
  type Choice,
  type DropSummary,
  type HeldFiles,
} from './packChoice';
import { recall, remember } from './remembered';
import {
  elided,
  groupHeading,
  hasHidden,
  leftWholeLine,
  NO_NUMBERING,
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
 * The confirmation every drop is answered with before anything is sent: where
 * it lands, what it carries, and Add or Cancel.
 *
 * Where it lands is said as the Library will hold it — the full path of the
 * folder each part of the drop goes under, see `destination.ts` — with the
 * levels the Library does not have yet marked as new, since a drop is also how
 * a folder comes to be.
 *
 * A drop of files alone says how many and how much, and that they are added one
 * by one. A drop holding a folder names each dropped folder with how many files
 * it holds and how much they come to, and asks what it goes in as: a Pack,
 * chosen unless somebody says otherwise, or the files one by one.
 *
 * A folder the Library already has where the drop lands is said to exist, and
 * the same answers are offered all the same. Adding goes as it always does: a
 * file whose path holds an Entry inside a Pack is refused on its own, and one
 * whose path holds a one-file Entry replaces that file. A Pack chosen here packs
 * only the files this drop carries — a replaced one among them — never the
 * other files already in that folder (spec: PK-17).
 *
 * Where the Library already holds a file at a path the drop is sent under, the
 * confirmation says so by name: how many files replace a one-file Entry, the
 * first few of them spelled out, and how many a Pack holds and so are refused.
 * The paths are the ones the drop will be sent under, so ticking a box below
 * that renames them changes what is said here too. Where nothing collides,
 * nothing more is said.
 *
 * For a drop holding a folder, two boxes offer to number the pages and the
 * volume folders of what was dropped, off unless the last drop ticked them.
 * Ticking one shows what it would rename before anything is sent, because a
 * rename is only worth having where it reads the names the way the person
 * meant — and Add carries the boxes beside the choice. Files dropped alone are
 * no folder's pages, and are sent under the names they were dropped with.
 *
 * Escape and a click outside it are Cancel, as they close any dialog — and
 * Cancel costs nothing, because nothing has been sent.
 */
export function DropConfirm({
  summary,
  files,
  destinations,
  held = NOTHING_HELD,
  onChoose,
}: {
  summary: DropSummary;
  /** The drop's files as dropped, which the preview is worked out from. */
  files: readonly Added[];
  /** Where each part of the drop lands, as `destinationsOf` names it. */
  destinations: readonly Destination[];
  /** What the Library holds where the drop lands, by path relative to it. */
  held?: HeldFiles;
  onChoose: (choice: Choice, numbering: Numbering) => void;
}) {
  const asked = summary.folders.length > 0;
  const [how, setHow] = useState<Exclude<Choice, 'cancel'>>(asked ? 'pack' : 'one_by_one');
  const [ticked, setTicked] = useState<Numbering>(() => ({
    pages: recall(REMEMBERED.pages),
    volumes: recall(REMEMBERED.volumes),
  }));
  // Files alone are numbered by nothing, whatever the last drop ticked.
  const numbering = asked ? ticked : NO_NUMBERING;
  const tick = (which: keyof Numbering, on: boolean) => {
    remember(REMEMBERED[which], on);
    setTicked((held) => ({ ...held, [which]: on }));
  };

  useEffect(() => {
    const closing = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onChoose('cancel', numbering);
      }
    };
    window.addEventListener('keydown', closing);
    return () => window.removeEventListener('keydown', closing);
  }, [onChoose, numbering]);

  const renumbered = useMemo(() => renumber(files, numbering), [files, numbering]);
  const groups = renumbered.groups;
  const collisions = useMemo(() => collisionsOf(renumbered.files, held), [renumbered, held]);
  const replaces = replacesLine(collisions);
  const refused = refusedLine(collisions);

  const labels = choiceLabels(summary);
  const question = questionOf(summary);
  const into = (folder: string | null) =>
    destinations.find((destination) => destination.folder === folder) ?? null;
  const loose = into(null);
  return (
    <div
      role="presentation"
      onClick={() => onChoose('cancel', numbering)}
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
        {asked && (
          <ul style={{ margin: 0, padding: 0, listStyle: 'none' }}>
            {summary.folders.map((folder) => {
              const destination = into(folder.name);
              return (
                <li key={folder.name} style={{ padding: '2px 0' }}>
                  {folderLine(folder)}
                  {destination !== null && <Where levels={destination.levels} />}
                  {folder.exists && (
                    <span style={{ ...INSET, color: COLOR.warn }}>{existsLine(folder)}</span>
                  )}
                </li>
              );
            })}
          </ul>
        )}
        {summary.loose > 0 && (
          <div>
            <span style={{ color: asked ? COLOR.dim : COLOR.text }}>{looseLine(summary)}</span>
            {loose !== null && <Where levels={loose.levels} />}
          </div>
        )}
        {replaces !== null && (
          <p style={{ margin: 0, color: COLOR.warn, overflowWrap: 'anywhere' }}>{replaces}</p>
        )}
        {refused !== null && <p style={{ margin: 0, color: COLOR.warn }}>{refused}</p>}
        {asked && (
          <fieldset style={{ margin: 0, padding: 0, border: 'none', display: 'flex', flexDirection: 'column', gap: 4 }}>
            <legend style={{ padding: 0, marginBottom: 4, color: COLOR.dim }}>add it as</legend>
            {(['pack', 'one_by_one'] as const).map((choice) => (
              <label key={choice} style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                <input
                  type="radio"
                  name="how"
                  checked={how === choice}
                  onChange={() => setHow(choice)}
                />
                {labels[choice]}
              </label>
            ))}
          </fieldset>
        )}
        {asked && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <label style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
              <input
                type="checkbox"
                checked={ticked.pages}
                onChange={(event) => tick('pages', event.target.checked)}
              />
              Number the pages
            </label>
            <label style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
              <input
                type="checkbox"
                checked={ticked.volumes}
                onChange={(event) => tick('volumes', event.target.checked)}
              />
              Number the volume folders
            </label>
          </div>
        )}
        {(numbering.pages || numbering.volumes) && <Preview groups={groups} />}
        {/* At the foot of the dialog and kept in view as it scrolls: a preview
            of a few hundred renames would otherwise push the one thing every
            drop needs answered out of reach. */}
        <div
          style={{
            position: 'sticky',
            bottom: -16,
            margin: '0 -16px -16px',
            padding: '10px 16px 16px',
            background: COLOR.panel,
            display: 'flex',
            justifyContent: 'flex-end',
            gap: 8,
          }}
        >
          <button type="button" onClick={() => onChoose('cancel', numbering)} style={CHOICE}>
            {CANCEL}
          </button>
          <button
            type="button"
            autoFocus
            onClick={() => onChoose(how, numbering)}
            style={{ ...CHOICE, borderColor: COLOR.added }}
          >
            {ADD}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * Where one part of the drop lands, as the Library will hold it, with the
 * levels it does not have yet drawn in the colour of what is on its way in and
 * said in words below.
 */
function Where({ levels }: { levels: readonly Level[] }) {
  const made = newLine(levels);
  return (
    <>
      <span style={{ ...INSET, overflowWrap: 'anywhere' }}>
        <span style={{ color: COLOR.dim }}>goes into </span>
        {levels.length === 0
          ? pathLine(levels)
          : levels.map((level) => (
              <span
                key={level.path}
                data-new={level.exists ? undefined : ''}
                style={level.exists ? undefined : { color: COLOR.added, fontStyle: 'italic' }}
              >
                {`${level.name}/`}
              </span>
            ))}
      </span>
      {made !== null && <span style={{ ...INSET, color: COLOR.added }}>{made}</span>}
    </>
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

/** A line under the one it belongs to, set in past the folder's mark. */
const INSET: CSSProperties = { display: 'block', paddingLeft: 22 };

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
