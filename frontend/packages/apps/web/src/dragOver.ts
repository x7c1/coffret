// Whether files are being dragged over a drop target: the tree, which answers
// for each folder row under the pointer and for the rest of it alike.

import { useState, type DragEvent } from 'react';

/** What a drop target knows of the drag over it, and the handlers that know it. */
export interface DragOver {
  /** Whether something is being dragged over the target right now. */
  dragged: boolean;
  /**
   * Whether what is being dragged carries files, as the browser says on the
   * way in. Only such a drag gets the line saying what letting go will do: a
   * selection of text or a link dropped here adds nothing (the drop says "that
   * drop carried no files"), and a line promising otherwise would be wrong.
   */
  carriesFiles: boolean;
  onDragEnter: (event: DragEvent) => void;
  onDragOver: (event: DragEvent) => void;
  onDragLeave: () => void;
  /** The drag is over: it was let go, and the target stops saying anything. */
  ended: () => void;
}

/**
 * The drag over one target.
 *
 * A `dragenter` and a `dragleave` fire for every element the pointer crosses
 * inside the target, so this is counted rather than set: a `dragleave` off one
 * child onto the next would otherwise take the outline away in the middle of
 * the drag.
 *
 * Both `dragenter` and every `dragover` are prevented. The browser's own default
 * for a dropped file is to navigate to it, which would replace the explorer with
 * the picture — and a target whose `dragover` is not prevented is one the
 * browser will not drop on at all.
 */
export function useDragOver(): DragOver {
  const [over, setOver] = useState(0);
  const [carriesFiles, setCarriesFiles] = useState(false);
  return {
    dragged: over > 0,
    carriesFiles,
    onDragEnter: (event) => {
      event.preventDefault();
      setOver((crossed) => crossed + 1);
      setCarriesFiles(event.dataTransfer.types.includes('Files'));
    },
    onDragOver: (event) => event.preventDefault(),
    onDragLeave: () => setOver((crossed) => Math.max(0, crossed - 1)),
    ended: () => setOver(0),
  };
}
