import { useEffect, type CSSProperties, type FormEvent, type ReactNode } from 'react';

import { COLOR } from './theme';

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
  onCancel,
  onSubmit,
  actions,
  width = 440,
  fill = false,
  children,
}: {
  /** The question, as the heading and as what the dialog is read out as. */
  title: string;
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
      <h2 style={{ margin: 0, fontSize: 15, fontWeight: 600, overflowWrap: 'anywhere' }}>{title}</h2>
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
      width: `min(${width}px, calc(100vw - 32px))`,
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
        padding: '8px 10px',
        borderRadius: 4,
        border: `1px solid ${tone === 'warn' ? COLOR.warn : COLOR.border}`,
        background: tone === 'warn' ? COLOR.warnGround : COLOR.ground,
        color: tone === 'warn' ? COLOR.warn : COLOR.text,
      }}
    >
      {children}
    </div>
  );
}

/** An answer that goes nowhere: Cancel, Close. */
export const BUTTON: CSSProperties = {
  border: `1px solid ${COLOR.border}`,
  background: COLOR.panel,
  color: COLOR.text,
  font: 'inherit',
  fontSize: 13,
  padding: '5px 14px',
  borderRadius: 4,
  cursor: 'pointer',
};

/** The answer that goes ahead. */
export const PRIMARY: CSSProperties = {
  ...BUTTON,
  border: `1px solid ${COLOR.selected}`,
  background: COLOR.selected,
};

/** The answer that goes ahead and takes something away. */
export const DANGER: CSSProperties = {
  ...BUTTON,
  border: `1px solid ${COLOR.refused}`,
  background: COLOR.refusedGround,
  color: COLOR.refused,
};

const BACKDROP: CSSProperties = {
  position: 'fixed',
  inset: 0,
  zIndex: 30,
  background: 'rgba(0, 0, 0, 0.55)',
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
  gap: 10,
  padding: 18,
  background: COLOR.panel,
  border: `1px solid ${COLOR.border}`,
  borderRadius: 8,
  boxShadow: '0 8px 32px rgba(0, 0, 0, 0.5)',
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
