// Nesting the Library's flat folder list, kept free of DOM so it is unit
// testable.

/** One folder of the Library, with the folders directly inside it. */
export interface FolderNode {
  /** Its last path component, which is what it is called. */
  name: string;
  /** Where in the Library it stands. */
  path: string;
  children: FolderNode[];
}

/**
 * The folders of the Library, nested.
 *
 * `GET /api/folders` answers flat and sorted — every path a separator implies,
 * each named in full — because the Library has no folders to nest: they are an
 * implication of the Entry Paths rather than something anything recorded. The
 * tree is this screen's arrangement of them, which is why it is built here.
 *
 * The order the server answered in is kept, at every level: it is the byte
 * order of the canonical paths, and re-sorting it would be this screen
 * disagreeing with every other device about what order a Library is in.
 *
 * A folder whose parent is not in the list still gets one. The server sends
 * every ancestor, so this never happens against a Library — but a tree missing
 * its own root is a worse answer than a tree with a folder in it that was
 * inferred, and the alternative is dropping the folder on the floor.
 */
export function nest(folders: readonly string[]): FolderNode[] {
  const roots: FolderNode[] = [];
  const byPath = new Map<string, FolderNode>();
  for (const path of folders) {
    ensure(path, byPath, roots);
  }
  return roots;
}

/** The node for one path, making it and everything above it if need be. */
function ensure(
  path: string,
  byPath: Map<string, FolderNode>,
  roots: FolderNode[],
): FolderNode {
  const held = byPath.get(path);
  if (held !== undefined) {
    return held;
  }
  const cut = path.lastIndexOf('/');
  const node: FolderNode = {
    name: cut === -1 ? path : path.slice(cut + 1),
    path,
    children: [],
  };
  byPath.set(path, node);
  if (cut === -1) {
    roots.push(node);
  } else {
    ensure(path.slice(0, cut), byPath, roots).children.push(node);
  }
  return node;
}

/**
 * Every folder on the way down to one, the outermost first.
 *
 * What the tree opens to when the screen is restored from the URL: a folder
 * several components deep is only reachable with each of its ancestors
 * expanded.
 */
export function ancestry(path: string): string[] {
  if (path === '') {
    return [];
  }
  const folders: string[] = [];
  let cut = path.indexOf('/');
  while (cut !== -1) {
    folders.push(path.slice(0, cut));
    cut = path.indexOf('/', cut + 1);
  }
  folders.push(path);
  return folders;
}
