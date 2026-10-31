import { useEffect, useMemo, useRef, useState, type CSSProperties } from 'react';

import type { Added } from '@coffret/api';

import { onDiskLine, newLine, type Destination, type Level } from './destination';
import { BUTTON, Dialog, LINE, PRIMARY } from './Dialog';
import { notTakenNotice, type NotTaken } from './drop';
import { systemFilesLine, type SystemFile } from './systemFiles';
import {
  ADD,
  addTitle,
  CANCEL,
  collisionsOf,
  existsLine,
  foldersReached,
  incomingLine,
  incomingOf,
  MARK_LABEL,
  NOTHING_HELD,
  refusedLine,
  replacesLine,
  STORE_CHOICES,
  totalLine,
  type Choice,
  type DropSummary,
  type HeldFiles,
} from './packChoice';
import { recall, remember } from './remembered';
import {
  corrected,
  EDITED,
  editedPath,
  groupHeading,
  leftWholeLine,
  LINES_DRAWN,
  misorderedLine,
  NO_EDITS,
  NO_NUMBERING,
  notDrawn,
  previewOf,
  refusalLine,
  renumber,
  UNDO,
  unchangedLine,
  type Edits,
  type Folded,
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

/** How many incoming items the confirmation names before it counts the rest. */
const ITEMS_SHOWN = 8;

/**
 * The confirmation every add is answered with before anything is sent —
 * files picked with the header's **+** and files dropped onto the tree alike:
 * `Add to <path>`, what the drop carries, and Add or Cancel.
 *
 * Under the heading, the whole drop counted — how many files and how much —
 * and then the incoming items: each dropped folder with what it holds, and
 * each file dropped loose, marked `New folder`, `New` or `Replaces` as it is
 * to the folder it lands in (see [`incomingOf`](./packChoice)). A folder the
 * Library already has there carries no mark and says so instead: the files in
 * it stay, and a dropped file at the path of one of them replaces it or is
 * refused. Where the folder added to is not one the Library holds yet — a
 * folder prepared with "New folder…", say — a line says the drop makes it.
 *
 * A drop holding a folder is asked how it is stored: **One Pack**, chosen
 * unless somebody says otherwise, or **Separate files**, each with a line
 * saying what it is for. A Pack chosen here packs only the files this drop
 * carries, never the other files already in that folder (spec: PK-17).
 *
 * The one-line notes, each shown only where it applies: what the desktop app
 * met in the drop and did not take (spec: LA-26); what either walk left out
 * because an operating system keeps it, in the dim colour, since such files
 * are in almost every drop (spec: EP-17); the files that replace a one-file
 * Entry, the first few by name; and how many a Pack holds the path of and so
 * are refused. The paths are the ones the drop will be sent under, so ticking
 * a box below that renames them changes what is said here too.
 *
 * For a drop holding a folder, two boxes offer to number the pages and the
 * volume folders, off unless the last drop ticked them. Ticking one shows
 * what it would rename inside the confirmation, before anything is sent: a
 * header line, `<n> renamed · <m> to check`, and each folder's names in the
 * order they are read in, where the lines that do not follow the rule — a
 * name left unnumbered, a two-page spread, a name corrected by hand — are
 * always shown and tinted, and the regular ones between them fold into
 * `… <n> more …` (see [`folded`](./renumber)). **Show all** lists every line,
 * and grows the dialog to the window's height less a margin, the preview
 * taking the height that is left and scrolling inside it. Folded or not, the
 * preview draws a bounded number of rows and counts the rest, with a button
 * that draws more, so that a drop of tens of thousands of files opens at once.
 * Files dropped alone are no folder's pages, and are sent under the names they
 * were dropped with.
 *
 * Where the rule read a name otherwise than the person meant, they correct it
 * here: clicking a new name opens it for editing — the last component only,
 * since the name is what is corrected, not where it lands — and Enter or
 * leaving the field takes it, Escape puts the field away without it. While
 * the field is open its Escape is not the dialog's Cancel and its Enter is not
 * Add. A name that cannot be held where it lands — empty, with `/` in it, `.`
 * or `..`, or one a name beside it already is — is not taken: the field stays
 * open and says why, and Add sends the names last taken. A name that breaks
 * the order the names beside it are read in is taken and said to break it,
 * since it may be a typo. A corrected line is marked as such and offers Undo,
 * which puts back the rule's name. A corrected volume folder carries the
 * pages under it, in the lines below it and in the paths sent. What the
 * Library holds under the folder it now names is asked for as the corrections
 * are made (`holding`), and Add waits for the answer, so a replacement or
 * refusal there is said as any other is — beside Add as well as above, where
 * a long preview may have scrolled it away. Add carries the corrections beside
 * the boxes. They belong to this question only: ticking or unticking either
 * box starts again from the rule's names, and nothing of them is kept for the
 * next drop.
 *
 * Escape and a click outside it are Cancel, as they close any dialog — and
 * Cancel costs nothing, because nothing has been sent.
 */
export function DropConfirm({
  landing,
  summary,
  files,
  destinations,
  held = NOTHING_HELD,
  notTaken = [],
  systemFiles = [],
  holding,
  onChoose,
}: {
  /** The folder the drop is added to; the Library root is the empty string. */
  landing: string;
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
  // Whether the preview lists every line, which grows the dialog with it.
  const [all, setAll] = useState(false);
  const tick = (which: keyof Numbering, on: boolean) => {
    remember(REMEMBERED[which], on);
    setTicked((before) => ({ ...before, [which]: on }));
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
  const cancel = () => onChoose('cancel', numbering, edits);

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
  // What the read under the corrected folder names turned up, said again
  // beside Add: the lines above may be scrolled out of view by a long preview
  // while the person waits for Add to come back.
  const found = useMemo(
    () => collisionsOf(reached === '' ? [] : renumbered.files, heldCorrected),
    [renumbered, heldCorrected, reached],
  );
  const foundLines = [replacesLine(found), refusedLine(found)].filter(
    (line): line is string => line !== null,
  );
  const left = notTakenNotice(notTaken);
  const kept = systemFilesLine(systemFiles);

  const items = incomingOf(summary, files, known);
  const shownItems = items.slice(0, ITEMS_SHOWN);
  const into = (folder: string | null) =>
    destinations.find((destination) => destination.folder === folder) ?? null;
  // The levels down to the folder added to: the loose files' destination, or a
  // dropped folder's without its own name.
  const landingLevels = into(null)?.levels ?? destinations[0]?.levels.slice(0, -1) ?? [];
  const title = addTitle(landing);
  return (
    <Dialog
      title={title}
      onCancel={cancel}
      width={560}
      fill={all}
      actions={
        <>
          {(reading || foundLines.length > 0) && (
            <div
              aria-live="polite"
              style={{ marginRight: 'auto', display: 'flex', flexDirection: 'column', gap: 2, overflowWrap: 'anywhere' }}
            >
              {reading ? (
                <span style={{ color: COLOR.dim }}>{READING}</span>
              ) : (
                foundLines.map((line) => (
                  <span key={line} style={{ color: COLOR.warn }}>
                    {line}
                  </span>
                ))
              )}
            </div>
          )}
          <button type="button" onClick={cancel} style={BUTTON}>
            {CANCEL}
          </button>
          <button
            type="button"
            autoFocus
            disabled={reading}
            onClick={() => onChoose(how, numbering, edits)}
            style={PRIMARY}
          >
            {ADD}
          </button>
        </>
      }
    >
      <p style={{ ...LINE, color: COLOR.dim }}>{totalLine(summary)}</p>
      <Making levels={landingLevels} />
      <ul aria-label="incoming" style={{ margin: 0, padding: 0, listStyle: 'none', display: 'flex', flexDirection: 'column', gap: 4 }}>
        {shownItems.map((item) => {
          const level = item.folder ? into(item.name)?.levels.at(-1) : undefined;
          return (
            <li key={`${item.folder ? 'folder' : 'file'}:${item.name}`}>
              <span style={{ display: 'flex', gap: 8, alignItems: 'baseline', minWidth: 0 }}>
                {item.mark !== null && (
                  <span style={item.mark === 'replaces' ? MARK_REPLACES : MARK}>{MARK_LABEL[item.mark]}</span>
                )}
                <span style={{ overflowWrap: 'anywhere' }}>
                  {item.folder ? '📁 ' : ''}
                  {incomingLine(item)}
                </span>
              </span>
              {item.folder && item.mark === null && (
                <span style={{ ...INSET, color: COLOR.warn }}>{existsLine(item)}</span>
              )}
              {level?.state === 'on_disk' && (
                <span style={{ ...INSET, color: COLOR.accentDeep }}>{onDiskLine([level])}</span>
              )}
            </li>
          );
        })}
        {items.length > shownItems.length && (
          <li style={{ color: COLOR.dim }}>… and {items.length - shownItems.length} more</li>
        )}
      </ul>
      {left !== null && <p style={{ ...LINE, color: COLOR.warn, overflowWrap: 'anywhere' }}>{left}</p>}
      {kept !== null && <p style={{ ...LINE, color: COLOR.dim, overflowWrap: 'anywhere' }}>{kept}</p>}
      {replaces !== null && (
        <p style={{ ...LINE, color: COLOR.warn, overflowWrap: 'anywhere' }}>{replaces}</p>
      )}
      {refused !== null && <p style={{ ...LINE, color: COLOR.warn }}>{refused}</p>}
      {reading && <p style={{ ...LINE, color: COLOR.dim }}>{READING}</p>}
      {asked && (
        <fieldset style={{ margin: 0, padding: 0, border: 'none', display: 'flex', flexDirection: 'column', gap: 6 }}>
          <legend style={{ padding: 0, marginBottom: 4, color: COLOR.dim }}>Store the folder as</legend>
          {(['pack', 'one_by_one'] as const).map((choice) => (
            <label key={choice} style={{ display: 'flex', gap: 8, alignItems: 'baseline' }}>
              <input type="radio" name="how" checked={how === choice} onChange={() => setHow(choice)} />
              <span>
                {STORE_CHOICES[choice].label}
                <span style={{ color: COLOR.dim }}> — {STORE_CHOICES[choice].note}</span>
              </span>
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
          all={all}
          onAll={setAll}
          correct={correct}
        />
      )}
    </Dialog>
  );
}

/**
 * What the confirmation says of the levels down to the folder added to that
 * the Library does not hold yet: the ones the drop makes, and the ones that
 * stand on this device's disk and go in with it. Nothing where every one is
 * the Library's.
 */
function Making({ levels }: { levels: readonly Level[] }) {
  const made = newLine(levels);
  const standing = onDiskLine(levels);
  return (
    <>
      {standing !== null && <p style={{ ...LINE, color: COLOR.accentDeep }}>{standing}</p>}
      {made !== null && <p style={{ ...LINE, color: COLOR.accentDeep }}>{made}</p>}
    </>
  );
}

/**
 * What the ticked boxes would rename: a header line counting the renames and
 * the lines to check, with Show all, and group by group the lines in the order
 * they are read in — the irregular ones always, tinted, and the regular ones
 * between them folded (see `folded`). A new name shown is one the person can
 * correct, through `correct`.
 *
 * A drop may carry tens of thousands of files, and a line drawn is a control
 * of its own, so no more than {@link LINES_DRAWN} rows are drawn at first —
 * folded or not — and what is past them is counted, with a button that draws
 * that many more. The tally above counts every line all the same.
 *
 * With `all` the box takes the height the dialog has left and scrolls inside.
 */
function Preview({
  groups,
  all,
  onAll,
  correct,
}: {
  groups: readonly Group[];
  all: boolean;
  onAll: (all: boolean) => void;
  /** Corrects the new name of a dropped path, or puts it back where null, or says why not. */
  correct: (path: string, name: string | null) => Refusal | null;
}) {
  const plan = useMemo(() => previewOf(groups, all), [groups, all]);
  // How many of the plan's rows are drawn: a bound, which Show all and Show
  // less put back, so that unfolding a long drop does not draw all of it.
  const [drawn, setDrawn] = useState(LINES_DRAWN);
  const unfold = (to: boolean) => {
    setDrawn(LINES_DRAWN);
    onAll(to);
  };
  if (groups.length === 0) {
    return <p style={{ ...LINE, color: COLOR.dim }}>nothing in this drop is renamed</p>;
  }
  const shown = plan.rows.slice(0, drawn);
  const rest = notDrawn(plan.rows, shown.length);
  // The rows drawn, put back under the group each is of: a group's heading
  // always comes before its lines, so a new group starts at each heading.
  const blocks: { group: Group; entries: Folded[] }[] = [];
  for (const row of shown) {
    if ('heading' in row) {
      blocks.push({ group: row.group, entries: [] });
    } else {
      blocks[blocks.length - 1].entries.push(row.entry);
    }
  }
  return (
    <section
      aria-label="what is renamed"
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
        padding: 8,
        border: `1px solid ${COLOR.border}`,
        borderRadius: 4,
        overflowWrap: 'anywhere',
        ...(all ? { flex: '1 1 0', minHeight: 160, overflow: 'auto' } : {}),
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <span style={{ color: COLOR.dim, marginRight: 'auto' }}>{plan.tally}</span>
        {(all || plan.hidden) && (
          <button type="button" onClick={() => unfold(!all)} style={SMALL}>
            {all ? 'Show less' : 'Show all'}
          </button>
        )}
      </div>
      {blocks.map(({ group, entries }) => (
        <div key={`${group.kind}:${group.folder}`}>
          <p style={{ ...LINE, color: COLOR.dim }}>{groupHeading(group)}</p>
          {group.leftWhole !== null && (
            <p style={{ ...LINE, color: COLOR.warn }}>{leftWholeLine(group.leftWhole)}</p>
          )}
          <ul style={{ margin: 0, paddingLeft: 16 }}>
            {entries.map((entry, at) => {
              if ('more' in entry) {
                return (
                  <li key={`more:${at}`} style={{ color: COLOR.dim, listStyle: 'none' }}>
                    … {entry.more} more …
                  </li>
                );
              }
              const line = entry.line;
              return 'renamed' in line ? (
                <RenameLine
                  key={line.renamed.from}
                  rename={line.renamed}
                  path={editedPath(group, line.renamed)}
                  tinted={entry.irregular}
                  correct={correct}
                />
              ) : (
                <li
                  key={line.unchanged.from}
                  style={{ color: COLOR.dim, ...(entry.irregular ? TINTED : {}) }}
                >
                  {unchangedLine(line.unchanged)}
                </li>
              );
            })}
          </ul>
        </div>
      ))}
      {shown.length < plan.rows.length && (
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span style={{ color: COLOR.dim, marginRight: 'auto' }}>{notDrawnLine(rest)}</span>
          <button type="button" onClick={() => setDrawn((before) => before + LINES_DRAWN)} style={SMALL}>
            {`Show ${LINES_DRAWN} more`}
          </button>
        </div>
      )}
    </section>
  );
}

/** What stands in for the preview's lines that are not drawn. */
function notDrawnLine(lines: number): string {
  return `… and ${lines} more ${lines === 1 ? 'line' : 'lines'} not drawn`;
}

/**
 * One rename, `<from> → <to>`, whose new name is a control that opens it for
 * correcting. The field holds the last component only, with the stem selected
 * so that the extension is kept unless it is typed over.
 */
function RenameLine({
  rename,
  path,
  tinted,
  correct,
}: {
  rename: Rename;
  /** The dropped path the line's corrections are kept under. */
  path: string;
  /** Whether the line does not follow the rule, and is tinted to be looked at. */
  tinted: boolean;
  correct: (path: string, name: string | null) => Refusal | null;
}) {
  // What is being typed, or null while the name is shown rather than edited.
  const [draft, setDraft] = useState<string | null>(null);
  const [refusal, setRefusal] = useState<Refusal | null>(null);
  // Whether the field is still open, read where leaving it would take the
  // name: Escape and Enter close it before the field goes and its blur comes.
  const open = useRef(false);
  // Whether the name shown next takes the focus: where Enter, Escape or Undo
  // puts away the control that had it, which would leave the focus on
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
    <li style={tinted ? TINTED : undefined}>
      {rename.from} <span style={{ color: COLOR.faint }}>→</span> {parent}
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
            {UNDO}
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

/** What Add waits on after a volume folder's name is corrected. */
const READING = 'reading what the Library holds under the corrected folder names…';

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

/** A line under the one it belongs to, set in past the item's mark. */
const INSET: CSSProperties = { display: 'block', paddingLeft: 12, fontSize: 12 };

/** A line that does not follow the rule: tinted, so it is the one looked at. */
const TINTED: CSSProperties = { background: COLOR.warnGround, borderRadius: 3 };

/** An item's mark, a small label before its name. */
const MARK: CSSProperties = {
  flex: '0 0 auto',
  fontSize: 11,
  padding: '0 6px',
  borderRadius: 3,
  border: `1px solid ${COLOR.accentDeep}`,
  color: COLOR.accentDeep,
};

/** A mark that takes the place of a file the Library holds. */
const MARK_REPLACES: CSSProperties = { ...MARK, borderColor: COLOR.warn, color: COLOR.warn };

/** A button inside the body: Show all. */
const SMALL: CSSProperties = { ...BUTTON, fontSize: 12, padding: '1px 8px' };
