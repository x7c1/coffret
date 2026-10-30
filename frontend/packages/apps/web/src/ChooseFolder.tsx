import { useMemo, useState, type CSSProperties } from 'react';

import { browseFolders, mapFolder, type Mapped } from '@coffret/api';

import { BUTTON, Dialog, LINE, PRIMARY } from './Dialog';
import {
  BROWSE,
  browseTo,
  chooseLine,
  chooseTitle,
  descend,
  mapHere,
  pickerFor,
  toParent,
  typedOver,
  USE_THIS_FOLDER,
  type Asking,
  type Picker,
} from './mapping';
import { COLOR } from './theme';

/**
 * Choosing the folder of this device that keeps one top-level folder of the
 * Library, or the whole of it: opened from the banner over a folder none is
 * chosen for, and inside the add flow where adding needs one (spec: EP-9).
 *
 * `Choose a folder on this device for “<top-level folder>”`, a line saying
 * what the folder is for and how far it reaches, a path field with Browse…,
 * and *Use this folder*, which records the choice. Browse… opens the folders
 * of this device under the field, through the server (see
 * [`mapping`](./mapping)), starting at the path in the field or at the home
 * folder; going into one puts its path in the field. `Enter` in the field
 * browses the path typed, so a path can be typed and then looked into as well
 * as used outright.
 *
 * A refusal is said in it and it stays open; a choice recorded is handed on.
 */
export function ChooseFolder({
  prefix,
  onChosen,
  onClose,
}: {
  /** The top-level folder of the Library a folder is chosen for, and `null` for the whole Library. */
  prefix: string | null;
  onChosen: (answer: Mapped) => void;
  onClose: () => void;
}) {
  const [picker, setPicker] = useState<Picker>(() => pickerFor(prefix));
  const asking: Asking = useMemo(
    () => ({
      browse: (path) => browseFolders(path),
      map: (localRoot, chosen) => mapFolder(localRoot, chosen),
      update: setPicker,
      mapped: onChosen,
    }),
    [onChosen],
  );
  const browse = () => {
    const typed = picker.typed.trim();
    void browseTo(typed === '' ? null : typed, asking);
  };

  const parent = picker.browsed?.parent ?? null;
  return (
    <Dialog
      title={chooseTitle(prefix)}
      onCancel={onClose}
      width={520}
      actions={
        <>
          <button type="button" onClick={onClose} style={BUTTON}>
            Cancel
          </button>
          <button
            type="button"
            disabled={picker.busy}
            onClick={() => void mapHere(picker, asking)}
            style={PRIMARY}
          >
            {USE_THIS_FOLDER}
          </button>
        </>
      }
    >
      <p style={{ ...LINE, color: COLOR.dim }}>{chooseLine(prefix)}</p>
      <form
        style={{ display: 'flex', gap: 8 }}
        onSubmit={(event) => {
          event.preventDefault();
          browse();
        }}
      >
        <input
          aria-label="Folder on this device"
          placeholder="/path/to/a/folder"
          autoFocus
          value={picker.typed}
          onChange={(event) => setPicker((held) => typedOver(held, event.target.value))}
          spellCheck={false}
          style={{ ...FIELD, flex: 1, minWidth: 0 }}
        />
        <button type="button" disabled={picker.busy} onClick={browse} style={BUTTON}>
          {BROWSE}
        </button>
      </form>
      {picker.browsed !== null && (
        <div role="list" aria-label="Folders on this device" style={FOLDERS}>
          {parent !== null && (
            <button
              type="button"
              role="listitem"
              disabled={picker.busy}
              onClick={() => void toParent(picker, asking)}
              style={ROW}
            >
              ↑ {parent}
            </button>
          )}
          {picker.browsed.folders.map((folder) => (
            <button
              key={folder.path}
              type="button"
              role="listitem"
              disabled={picker.busy}
              onClick={() => void descend(folder, asking)}
              style={ROW}
            >
              ▸ {folder.name}
            </button>
          ))}
          {picker.browsed.folders.length === 0 && (
            <p style={{ margin: 0, padding: '5px 10px', color: COLOR.dim }}>no folders in here</p>
          )}
        </div>
      )}
      {picker.refused !== null && <p style={{ ...LINE, color: COLOR.refused }}>{picker.refused}</p>}
    </Dialog>
  );
}

const FIELD: CSSProperties = {
  border: `1px solid ${COLOR.border}`,
  background: COLOR.ground,
  color: COLOR.text,
  font: 'inherit',
  padding: '5px 8px',
  borderRadius: 4,
};

/** The browsed folder's folders, in a box of their own that scrolls. */
const FOLDERS: CSSProperties = {
  minHeight: 120,
  maxHeight: 260,
  overflow: 'auto',
  border: `1px solid ${COLOR.rowRule}`,
  borderRadius: 4,
};

const ROW: CSSProperties = {
  display: 'block',
  width: '100%',
  textAlign: 'left',
  border: 'none',
  borderBottom: `1px solid ${COLOR.rowRule}`,
  background: 'transparent',
  color: COLOR.text,
  font: 'inherit',
  padding: '5px 10px',
  cursor: 'pointer',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
};
