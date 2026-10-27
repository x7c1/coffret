import { useEffect, useMemo, useRef, useState, type CSSProperties } from 'react';

import type { Added } from '@coffret/api';

import { newLine, onDiskLine, pathLine, type Destination, type Level } from './destination';
import { notTakenNotice, type NotTaken } from './drop';
import { systemFilesLine, type SystemFile } from './systemFiles';
import {
  ADD,
  CANCEL,
  choiceLabels,
  collisionsOf,
  existsLine,
  folderLine,
  foldersReached,
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
  corrected,
  EDITED,
  editedPath,
  elided,
  groupHeading,
  hasHidden,
  leftWholeLine,
  misorderedLine,
  NO_EDITS,
  NO_NUMBERING,
  PUT_BACK,
  refusalLine,
  renumber,
  unchangedLine,
  type Edits,
  type Group,
  type Numbering,
  type Refusal,
  type Rename,
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
 * Where the desktop app met something in the drop and did not take it — a
 * symbolic link, a name coffret keeps for itself — the confirmation says so in
 * one line, whether the drop is added straight away or asked what it goes in
 * as: what is listed above is what was taken, and what was let go of is more
 * than that (spec: LA-26). A browser's drop has nothing to say here.
 *
 * What either walk left out because an operating system keeps it — `.DS_Store`,
 * `Thumbs.db`, `._` files — is neither counted nor sized above, and is said in
 * one line of its own in the dim colour, naming a few of the distinct names:
 * such files are in almost every drop, and saying them as the line above says
 * what was not taken would read as something gone wrong (spec: EP-17).
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
 * Where the rule read a name otherwise than the person meant, they correct it
 * here rather than untick the box and rename after the drop: clicking a new
 * name in the preview opens it for editing — the last component only, since
 * the name is what is corrected, not where it lands — and Enter or leaving the
 * field takes it, Escape puts the field away without it. While the field is
 * open its Escape is not the dialog's Cancel and its Enter is not Add. A name
 * that cannot be held where it lands — empty, with `/` in it, `.` or `..`, or
 * one a name beside it already is — is not taken: the field stays open and
 * says why, and Add sends the names last taken. A name that breaks the order
 * the names beside it are read in is taken and said to break it, since it may
 * be a typo. A corrected line is marked as such and offers to put back the
 * rule's name. A corrected volume folder carries the pages under it, in the
 * lines below it and in the paths sent. What the Library holds under the
 * folder it now names is asked for as the corrections are made (`holding`),
 * and Add waits for the answer, so a replacement or refusal there is said as
 * any other is. Add carries the corrections beside the boxes. They belong to
 * this question only: ticking or unticking either box starts again from the
 * rule's names, and nothing of them is kept for the next drop.
 *
 * Escape and a click outside it are Cancel, as they close any dialog — and
 * Cancel costs nothing, because nothing has been sent.
 */
export function DropConfirm({
  summary,
  files,
  destinations,
  held = NOTHING_HELD,
  notTaken = [],
  systemFiles = [],
  holding,
  onChoose,
}: {
  summary: DropSummary;
  /** The drop's files as dropped, which the preview is worked out from. */
  files: readonly Added[];
  /** Where each part of the drop lands, as `destinationsOf` names it. */
  destinations: readonly Destination[];
  /** What the Library holds where the drop lands, by path relative to it. */
  held?: HeldFiles;
  /** What the desktop app met in the drop and did not take; none in a browser. */
  notTaken?: readonly NotTaken[];
  /** What the drop left out because an operating system keeps it. */
  systemFiles?: readonly SystemFile[];
  /**
   * What the Library holds in the given folders, relative to where the drop
   * lands — asked where a corrected volume name sends files into a folder
   * `held` was not read from. It answers with nothing held, rather than
   * refusing, where a listing cannot be had.
   */
  holding?: (folders: readonly string[]) => Promise<HeldFiles>;
  onChoose: (choice: Choice, numbering: Numbering, edits: Edits) => void;
}) {
  const asked = summary.folders.length > 0;
  const [how, setHow] = useState<Exclude<Choice, 'cancel'>>(asked ? 'pack' : 'one_by_one');
  const [ticked, setTicked] = useState<Numbering>(() => ({
    pages: recall(REMEMBERED.pages),
    volumes: recall(REMEMBERED.volumes),
  }));
  // Files alone are numbered by nothing, whatever the last drop ticked.
  const numbering = asked ? ticked : NO_NUMBERING;
  // The person's corrections of the rule's names, which a box ticked or
  // unticked discards: the rule's names they corrected are other names then.
  const [edits, setEdits] = useState<Edits>(NO_EDITS);
  const tick = (which: keyof Numbering, on: boolean) => {
    remember(REMEMBERED[which], on);
    setTicked((held) => ({ ...held, [which]: on }));
    setEdits(NO_EDITS);
  };
  const correct = (path: string, name: string | null): Refusal | null => {
    const result = corrected(files, numbering, edits, path, name);
    if ('refused' in result) {
      return result.refused;
    }
    setEdits(result.edits);
    return null;
  };

  useEffect(() => {
    const closing = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onChoose('cancel', numbering, edits);
      }
    };
    window.addEventListener('keydown', closing);
    return () => window.removeEventListener('keydown', closing);
  }, [onChoose, numbering, edits]);

  const renumbered = useMemo(() => renumber(files, numbering, edits), [files, numbering, edits]);
  const groups = renumbered.groups;

  // A corrected volume name can send files into a folder no numbering would
  // have, which `held` was not read from: what the Library holds there is
  // asked for as the corrections are made, and kept beside `held` — both are
  // what the Library holds at a path, whichever answer is sent. The folders
  // are joined into one string so that the effect below asks again only when
  // they change, not on every new array.
  const reached = useMemo(() => {
    const corrects = groups.some(
      (group) => group.kind === 'volumes' && group.renamed.some((rename) => rename.ruled !== undefined),
    );
    return corrects ? foldersReached(renumbered.files).sort().join('\n') : '';
  }, [groups, renumbered]);
  const [heldCorrected, setHeldCorrected] = useState<HeldFiles>(NOTHING_HELD);
  // The folders last answered for. Until the answer for those reached now
  // comes, Add waits: the person would otherwise send without being told what
  // the corrected folders replace — as they would by clicking Add straight
  // from the field, whose leaving takes the name and asks.
  const [answered, setAnswered] = useState('');
  const reading = holding !== undefined && reached !== '' && answered !== reached;
  useEffect(() => {
    if (holding === undefined || reached === '') {
      return;
    }
    let current = true;
    void holding(reached.split('\n'))
      .then((found) => {
        if (current) {
          setHeldCorrected((known) => new Map([...known, ...found]));
        }
      })
      .finally(() => {
        if (current) {
          setAnswered(reached);
        }
      });
    return () => {
      current = false;
    };
  }, [holding, reached]);
  const known = useMemo(
    () => (heldCorrected.size === 0 ? held : new Map([...held, ...heldCorrected])),
    [held, heldCorrected],
  );
  const collisions = useMemo(() => collisionsOf(renumbered.files, known), [renumbered, known]);
  const replaces = replacesLine(collisions);
  const refused = refusedLine(collisions);
  const left = notTakenNotice(notTaken);
  const kept = systemFilesLine(systemFiles);

  const labels = choiceLabels(summary);
  const question = questionOf(summary);
  const into = (folder: string | null) =>
    destinations.find((destination) => destination.folder === folder) ?? null;
  const loose = into(null);
  return (
    <div
      role="presentation"
      onClick={() => onChoose('cancel', numbering, edits)}
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
        {left !== null && (
          <p style={{ margin: 0, color: COLOR.warn, overflowWrap: 'anywhere' }}>{left}</p>
        )}
        {kept !== null && (
          <p style={{ margin: 0, color: COLOR.dim, overflowWrap: 'anywhere' }}>{kept}</p>
        )}
        {replaces !== null && (
          <p style={{ margin: 0, color: COLOR.warn, overflowWrap: 'anywhere' }}>{replaces}</p>
        )}
        {refused !== null && <p style={{ margin: 0, color: COLOR.warn }}>{refused}</p>}
        {reading && (
          <p style={{ margin: 0, color: COLOR.dim }}>
            reading what the Library holds under the corrected folder names…
          </p>
        )}
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
        {/* Keyed by the boxes, so that ticking one also puts away a name
            being corrected: the corrections are discarded with it. */}
        {(numbering.pages || numbering.volumes) && (
          <Preview
            key={`${numbering.pages}:${numbering.volumes}`}
            groups={groups}
            correct={correct}
          />
        )}
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
          <button type="button" onClick={() => onChoose('cancel', numbering, edits)} style={CHOICE}>
            {CANCEL}
          </button>
          <button
            type="button"
            autoFocus
            disabled={reading}
            onClick={() => onChoose(how, numbering, edits)}
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
  const standing = onDiskLine(levels);
  return (
    <>
      <span style={{ ...INSET, overflowWrap: 'anywhere' }}>
        <span style={{ color: COLOR.dim }}>goes into </span>
        {levels.length === 0
          ? pathLine(levels)
          : levels.map((level) => (
              <span
                key={level.path}
                data-new={level.state === 'new' ? '' : undefined}
                data-on-disk={level.state === 'on_disk' ? '' : undefined}
                style={LEVEL[level.state]}
              >
                {`${level.name}/`}
              </span>
            ))}
      </span>
      {standing !== null && <span style={{ ...INSET, color: COLOR.added }}>{standing}</span>}
      {made !== null && <span style={{ ...INSET, color: COLOR.added }}>{made}</span>}
    </>
  );
}

/**
 * How a level is drawn: as the Library names it, or in the colour of what is on
 * its way in — slanted where the drop is what makes it, upright where it stands
 * on the disk already.
 */
const LEVEL: Record<Level['state'], CSSProperties | undefined> = {
  named: undefined,
  on_disk: { color: COLOR.added },
  new: { color: COLOR.added, fontStyle: 'italic' },
};

/**
 * What the ticked boxes would rename, group by group: both ends of each run of
 * renames and the names left as they are, with a way to see all of it. A new
 * name shown is one the person can correct, through `correct`.
 */
function Preview({
  groups,
  correct,
}: {
  groups: readonly Group[];
  /** Corrects the new name of a dropped path, or puts it back where null, or says why not. */
  correct: (path: string, name: string | null) => Refusal | null;
}) {
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
                <RenameLine
                  key={rename.from}
                  rename={rename}
                  path={editedPath(group, rename)}
                  correct={correct}
                />
              ))}
              {renamed.hidden > 0 && <li style={{ color: COLOR.dim }}>… {renamed.hidden} more …</li>}
              {renamed.tail.map((rename) => (
                <RenameLine
                  key={rename.from}
                  rename={rename}
                  path={editedPath(group, rename)}
                  correct={correct}
                />
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

/**
 * One rename, `<from> → <to>`, whose new name is a control that opens it for
 * correcting. The field holds the last component only, with the stem selected
 * so that the extension is kept unless it is typed over.
 */
function RenameLine({
  rename,
  path,
  correct,
}: {
  rename: Rename;
  /** The dropped path the line's corrections are kept under. */
  path: string;
  correct: (path: string, name: string | null) => Refusal | null;
}) {
  // What is being typed, or null while the name is shown rather than edited.
  const [draft, setDraft] = useState<string | null>(null);
  const [refusal, setRefusal] = useState<Refusal | null>(null);
  // Whether the field is still open, read where leaving it would take the
  // name: Escape and Enter close it before the field goes and its blur comes.
  const open = useRef(false);
  // Whether the name shown next takes the focus: where Enter, Escape or put
  // back puts away the control that had it, which would leave the focus on
  // nothing and the keyboard with no place in the dialog. Leaving the field
  // for something else keeps the focus there.
  const refocus = useRef(false);
  const cut = rename.to.lastIndexOf('/');
  const parent = rename.to.slice(0, cut + 1);
  const name = rename.to.slice(cut + 1);

  const start = () => {
    open.current = true;
    setDraft(name);
    setRefusal(null);
  };
  const take = (typed: string) => {
    if (!open.current) {
      return;
    }
    const refused = correct(path, typed);
    setRefusal(refused);
    if (refused === null) {
      open.current = false;
      setDraft(null);
    } else {
      refocus.current = false;
    }
  };
  const discard = () => {
    refocus.current = true;
    open.current = false;
    setDraft(null);
    setRefusal(null);
  };

  return (
    <li>
      {rename.from} → {parent}
      {draft === null ? (
        <button
          type="button"
          title="correct this name"
          ref={(shown) => {
            if (shown !== null && refocus.current) {
              refocus.current = false;
              shown.focus();
            }
          }}
          onClick={start}
          style={NAME}
        >
          {name}
        </button>
      ) : (
        <input
          type="text"
          aria-label={`new name of ${rename.from}`}
          aria-invalid={refusal !== null}
          autoFocus
          value={draft}
          onFocus={(event) => {
            const dot = event.target.value.lastIndexOf('.');
            event.target.setSelectionRange(0, dot > 0 ? dot : event.target.value.length);
          }}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            // The field's own keys: neither may reach the dialog, whose Escape
            // is Cancel.
            if (event.key === 'Enter') {
              event.preventDefault();
              event.stopPropagation();
              refocus.current = true;
              take(event.currentTarget.value);
            } else if (event.key === 'Escape') {
              event.preventDefault();
              event.stopPropagation();
              discard();
            }
          }}
          onBlur={(event) => take(event.currentTarget.value)}
          style={FIELD}
        />
      )}
      {draft === null && rename.ruled !== undefined && (
        <>
          {' '}
          <span style={{ color: COLOR.dim }}>{EDITED}</span>{' '}
          <button
            type="button"
            title={`back to ${rename.ruled}`}
            onClick={() => {
              refocus.current = true;
              correct(path, null);
            }}
            style={{ ...NAME, color: COLOR.dim }}
          >
            {PUT_BACK}
          </button>
        </>
      )}
      {refusal !== null && (
        <span style={{ display: 'block', color: COLOR.refused }}>{refusalLine(refusal)}</span>
      )}
      {draft === null && rename.misordered !== undefined && (
        <span style={{ display: 'block', color: COLOR.warn }}>
          {misorderedLine(rename.misordered)}
        </span>
      )}
    </li>
  );
}

/** A new name that opens for correcting: read as text, with a dotted line under it. */
const NAME: CSSProperties = {
  border: 'none',
  background: 'none',
  padding: 0,
  font: 'inherit',
  color: 'inherit',
  cursor: 'text',
  textDecoration: 'underline dotted',
  textUnderlineOffset: 2,
};

/** The field a new name is corrected in, as wide as the line lets it be. */
const FIELD: CSSProperties = {
  font: 'inherit',
  color: COLOR.text,
  background: COLOR.panel,
  border: `1px solid ${COLOR.border}`,
  borderRadius: 3,
  padding: '0 4px',
  minWidth: '12em',
};

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
