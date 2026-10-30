import { useEffect, type CSSProperties, type FormEvent, type ReactNode } from 'react';

import { LIBRARY_ROOT } from './location';
import { FOLDER_ICON_PATH } from './rowIcon';
import { COLOR, SHADOW } from './theme';

/**
 * The frame the explorer's questions stand in — "Delete…", "New folder…",
 * "Pack this folder…", the folder on this device chosen while adding, and the
 * confirmation of an add: a panel over the dimmed screen with the question as
 * its heading, what it has to say under it, and the answers in a row at the
 * bottom right, the one that goes ahead last.
 *
 * The panel is never taller than the window less a margin: what it has to say
 * scrolls between the heading and the answers, which stay in view. With
 * `fill` it is as tall as that from the start, for a body that holds a box
 * taking the rest of the height and scrolling inside it.
 *
 * Escape and a click outside it are `onCancel`, as they close any dialog. With
 * `onSubmit` the panel is a form, so `Enter` in its field is the submit button.
 */
export function Dialog({
  title,
  heading,
  onCancel,
  onSubmit,
  actions,
  width = 480,
  fill = false,
  children,
}: {
  /** The question, as the heading and as what the dialog is read out as. */
  title: string;
  /**
   * The heading as drawn, where it sets a name apart from the words around it
   * (see [`Place`](#Place)); `title` is still what the dialog is read out as.
   */
  heading?: ReactNode;
  onCancel: () => void;
  onSubmit?: (event: FormEvent) => void;
  /** The answers, in the order they stand left to right. */
  actions: ReactNode;
  /** How wide the panel is, where the window has the room. */
  width?: number;
  /** Whether the panel takes the window's height less a margin, whatever its body holds. */
  fill?: boolean;
  children: ReactNode;
}) {
  useEffect(() => {
    const closing = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        // Taken: the reader's sheet, open behind the dialog, closes on an
        // `Escape` nothing else took, and this one was the dialog's.
        event.preventDefault();
        onCancel();
      }
    };
    window.addEventListener('keydown', closing);
    return () => window.removeEventListener('keydown', closing);
  }, [onCancel]);

  const body = (
    <>
      <h2 style={{ margin: 0, fontSize: 16, fontWeight: 600, overflowWrap: 'anywhere' }}>{heading ?? title}</h2>
      <div style={BODY}>{children}</div>
      <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', alignItems: 'center', marginTop: 4 }}>
        {actions}
      </div>
    </>
  );
  const panel = {
    role: 'dialog',
    'aria-modal': true,
    'aria-label': title,
    onClick: (event: { stopPropagation: () => void }) => event.stopPropagation(),
    style: {
      ...PANEL,
      width: `min(${width}px, 100% - 32px)`,
      ...(fill ? { height: TALLEST } : {}),
    },
  } as const;
  return (
    <div role="presentation" onClick={onCancel} style={BACKDROP}>
      {onSubmit === undefined ? (
        <div {...panel}>{body}</div>
      ) : (
        <form {...panel} onSubmit={onSubmit}>
          {body}
        </form>
      )}
    </div>
  );
}

/**
 * A folder named in a dialog's heading, set apart from the words of it: on a
 * line of its own, a folder icon and the path in a tinted box, in the body's
 * size and weight, so a name such as "Series A" is not read as part of the
 * sentence. The Library root is named as the header names it.
 */
export function Place({ path }: { path: string }) {
  return (
    <span data-testid="place" style={PLACE}>
      <svg
        aria-hidden
        width={14}
        height={14}
        viewBox="0 0 24 24"
        fill={COLOR.folderFill}
        stroke={COLOR.folderStroke}
        strokeWidth={2}
        strokeLinejoin="round"
        style={{ flex: '0 0 auto' }}
      >
        <path d={FOLDER_ICON_PATH} />
      </svg>
      <span style={{ minWidth: 0, overflowWrap: 'anywhere' }}>{path === '' ? LIBRARY_ROOT : path}</span>
    </span>
  );
}

const PLACE: CSSProperties = {
  // A line of its own under the words of the heading, in the body's size and
  // weight: the heading asks, and this names where.
  display: 'flex',
  alignItems: 'center',
  gap: 8,
  width: 'fit-content',
  maxWidth: '100%',
  boxSizing: 'border-box',
  marginTop: 8,
  padding: '4px 10px',
  border: `1px solid ${COLOR.border}`,
  borderRadius: 6,
  background: COLOR.ground,
  fontSize: 14,
  fontWeight: 400,
};

/** A line of the dialog's body. */
export const LINE: CSSProperties = { margin: 0 };

/** A box set apart inside the body: a note to read before answering. */
export function Note({ tone, children }: { tone: 'warn' | 'plain'; children: ReactNode }) {
  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 4,
        padding: '10px 12px',
        borderRadius: 8,
        border: `1px solid ${tone === 'warn' ? COLOR.warnGround : COLOR.border}`,
        background: tone === 'warn' ? COLOR.warnGround : COLOR.ground,
        color: tone === 'warn' ? COLOR.warnDeep : COLOR.text,
      }}
    >
      {children}
    </div>
  );
}

/** An answer that goes nowhere: Cancel, Close. */
export const BUTTON: CSSProperties = {
  minHeight: 36,
  border: `1px solid ${COLOR.border}`,
  background: COLOR.panel,
  color: COLOR.text,
  font: 'inherit',
  fontSize: 13,
  padding: '0 16px',
  borderRadius: 8,
  cursor: 'pointer',
};

/** The answer that goes ahead: filled with the accent, as **Open** and the **+** are. */
export const PRIMARY: CSSProperties = {
  ...BUTTON,
  border: `1px solid ${COLOR.accent}`,
  background: COLOR.accent,
  color: COLOR.onAccent,
  fontWeight: 600,
};

/** The answer that goes ahead and takes something away. */
export const DANGER: CSSProperties = {
  ...BUTTON,
  border: `1px solid ${COLOR.refused}`,
  background: COLOR.refusedGround,
  color: COLOR.refused,
  fontWeight: 600,
};

const BACKDROP: CSSProperties = {
  position: 'fixed',
  inset: 0,
  zIndex: 30,
  background: COLOR.backdrop,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
};

/** The tallest a panel is: the window's height, less a margin above and below. */
const TALLEST = 'calc(100vh - 48px)';

const PANEL: CSSProperties = {
  maxHeight: TALLEST,
  boxSizing: 'border-box',
  display: 'flex',
  flexDirection: 'column',
  gap: 14,
  padding: 24,
  background: COLOR.panel,
  border: `1px solid ${COLOR.border}`,
  borderRadius: 12,
  boxShadow: SHADOW.dialog,
  color: COLOR.text,
  fontSize: 13,
  lineHeight: 1.45,
};

/** What the dialog says, between the heading and the answers: scrolled where it runs long. */
const BODY: CSSProperties = {
  flex: '1 1 auto',
  minHeight: 0,
  overflow: 'auto',
  display: 'flex',
  flexDirection: 'column',
  gap: 10,
};
