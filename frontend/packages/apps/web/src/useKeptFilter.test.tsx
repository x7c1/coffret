// The filter settings kept for this device by the server (spec: LA-43): read
// as the page opens, and every change kept — the name once typing pauses.

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { NO_FILTER, type FilterSettings } from './filter';
import { NAME_PAUSE_MS, useKeptFilter, type KeptFilter } from './useKeptFilter';

/** A store held in memory, whose read answers when the case says. */
function store(initial: FilterSettings) {
  let answer: (settings: FilterSettings) => void = () => {};
  const read = vi.fn(
    () =>
      new Promise<FilterSettings>((resolve) => {
        answer = resolve;
      }),
  );
  const write = vi.fn((settings: FilterSettings) => Promise.resolve(settings));
  const kept: KeptFilter = { read, write };
  return { kept, read, write, answer: () => answer(initial) };
}

const KEPT: FilterSettings = { name: 'star', recentOn: true, recent: 3 };

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe('useKeptFilter', () => {
  it('starts from no filter and takes the kept settings once answered', async () => {
    const held = store(KEPT);
    const { result } = renderHook(() => useKeptFilter(true, held.kept));
    expect(result.current.settings).toEqual(NO_FILTER);
    await act(async () => held.answer());
    expect(result.current.settings).toEqual(KEPT);
    expect(held.write).not.toHaveBeenCalled();
  });

  // LA-43: kept settings the same as those on screen are left as they stand.
  it('leaves the settings on screen as they are where the kept ones equal them', async () => {
    const held = store({ ...NO_FILTER });
    const { result } = renderHook(() => useKeptFilter(true, held.kept));
    const before = result.current.settings;
    await act(async () => held.answer());
    // The same object, so nothing drawn from it — the tree, its focus, the reader — is drawn anew.
    expect(result.current.settings).toBe(before);
  });

  // LA-43: the name is kept once typing in it pauses, not at every key.
  it('keeps the name once typing pauses, not once per key', async () => {
    const held = store(NO_FILTER);
    const { result } = renderHook(() => useKeptFilter(true, held.kept));
    await act(async () => held.answer());
    for (const name of ['s', 'st', 'sta', 'star']) {
      act(() => result.current.change((now) => ({ ...now, name })));
      await act(async () => vi.advanceTimersByTime(NAME_PAUSE_MS / 2));
    }
    expect(held.write).not.toHaveBeenCalled();
    await act(async () => vi.advanceTimersByTime(NAME_PAUSE_MS));
    expect(held.write).toHaveBeenCalledTimes(1);
    expect(held.write).toHaveBeenCalledWith({ ...NO_FILTER, name: 'star' });
  });

  // LA-43: the name is kept as the field is left.
  it('keeps the name at once as the field is left', async () => {
    const held = store(NO_FILTER);
    const { result } = renderHook(() => useKeptFilter(true, held.kept));
    await act(async () => held.answer());
    act(() => result.current.change({ ...NO_FILTER, name: 'cat' }));
    await act(async () => result.current.nameLeft());
    expect(held.write).toHaveBeenCalledTimes(1);
    expect(held.write).toHaveBeenCalledWith({ ...NO_FILTER, name: 'cat' });
    // Nothing is left waiting to be kept twice.
    await act(async () => vi.advanceTimersByTime(NAME_PAUSE_MS * 2));
    expect(held.write).toHaveBeenCalledTimes(1);
  });

  // LA-43: a change of Recent or its count is kept at once.
  it('keeps Recent and its count at once', async () => {
    const held = store(NO_FILTER);
    const { result } = renderHook(() => useKeptFilter(true, held.kept));
    await act(async () => held.answer());
    await act(async () => result.current.change((now) => ({ ...now, recentOn: true })));
    expect(held.write).toHaveBeenLastCalledWith({ ...NO_FILTER, recentOn: true });
    await act(async () => result.current.change((now) => ({ ...now, recent: 4 })));
    expect(held.write).toHaveBeenLastCalledWith({ ...NO_FILTER, recentOn: true, recent: 4 });
    expect(held.write).toHaveBeenCalledTimes(2);
  });

  // LA-43: a change that could not be kept is said in the console and leaves
  // the filter on screen as it is.
  it('says a failed keep in the console and leaves the filter on screen', async () => {
    const held = store(NO_FILTER);
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    held.write.mockRejectedValueOnce(new Error('refused'));
    const { result } = renderHook(() => useKeptFilter(true, held.kept));
    await act(async () => held.answer());
    const changed = { ...NO_FILTER, recentOn: true, recent: 4 };
    await act(async () => result.current.change(changed));
    expect(held.write).toHaveBeenCalledWith(changed);
    expect(warn).toHaveBeenCalledTimes(1);
    expect(result.current.settings).toEqual(changed);
    warn.mockRestore();
  });

  // LA-43: kept settings that could not be read are said in the console and
  // leave the filter on screen as it is.
  it('says a failed read in the console and leaves the filter on screen', async () => {
    const held = store(KEPT);
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    held.read.mockRejectedValueOnce(new Error('refused'));
    const { result } = renderHook(() => useKeptFilter(true, held.kept));
    await act(async () => {});
    expect(warn).toHaveBeenCalledTimes(1);
    expect(result.current.settings).toEqual(NO_FILTER);
    warn.mockRestore();
  });

  // LA-43: a change made before the kept settings are answered wins over them.
  it('lets a change made before the answer win over it', async () => {
    const held = store(KEPT);
    const { result } = renderHook(() => useKeptFilter(true, held.kept));
    act(() => result.current.change({ ...NO_FILTER, recentOn: true }));
    await act(async () => held.answer());
    expect(result.current.settings).toEqual({ ...NO_FILTER, recentOn: true });
  });

  // LA-43: once unlocked, the kept settings are read again where nothing was
  // read or set.
  it('reads again once the Library is unlocked where a lock refused the first read', async () => {
    const held = store(KEPT);
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    held.read.mockRejectedValueOnce(new Error('locked'));
    const { result, rerender } = renderHook(({ open }) => useKeptFilter(open, held.kept), {
      initialProps: { open: true },
    });
    await act(async () => {});
    expect(warn).toHaveBeenCalled();
    rerender({ open: false });
    rerender({ open: true });
    expect(held.read).toHaveBeenCalledTimes(2);
    await act(async () => held.answer());
    expect(result.current.settings).toEqual(KEPT);
    warn.mockRestore();
  });

  // LA-43: once unlocked, a change whose keeping was refused is kept again.
  it('keeps again once the Library is unlocked what a lock refused to keep', async () => {
    const held = store(NO_FILTER);
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    held.write.mockRejectedValueOnce(new Error('locked'));
    const { result, rerender } = renderHook(({ open }) => useKeptFilter(open, held.kept), {
      initialProps: { open: true },
    });
    await act(async () => held.answer());
    const changed = { ...NO_FILTER, recentOn: true, recent: 4 };
    await act(async () => result.current.change(changed));
    expect(warn).toHaveBeenCalledTimes(1);
    rerender({ open: false });
    expect(held.write).toHaveBeenCalledTimes(1);
    await act(async () => rerender({ open: true }));
    expect(held.write).toHaveBeenCalledTimes(2);
    expect(held.write).toHaveBeenLastCalledWith(changed);
    expect(held.read).toHaveBeenCalledTimes(1);

    // Kept now: a later unlock sends nothing more.
    rerender({ open: false });
    await act(async () => rerender({ open: true }));
    expect(held.write).toHaveBeenCalledTimes(2);
    warn.mockRestore();
  });
});
