import { fail, type CoffretErrorCode } from '../errors.js';

/**
 * Holds a Library's name, as a record or a Snapshot carries it, to a name's own
 * rules (spec: FM-15, FM-16): it is not empty, and it holds no control
 * character. Nothing else is asked of it — the name names no directory and
 * nothing on Storage, so a `/` or a `.` is an ordinary character in one.
 *
 * The refusal says what is wrong and never quotes the name, which is something
 * a person chose (spec: EL-1).
 */
export function requireLibraryName(name: string, code: CoffretErrorCode): string {
  if (name.length === 0) {
    fail(code, 'library_name is not a Library name: it is empty');
  }
  // What Rust's `char::is_control` answers: the C0 and C1 control characters.
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f-\u009f]/u.test(name)) {
    fail(code, 'library_name is not a Library name: it holds a control character');
  }
  return name;
}
