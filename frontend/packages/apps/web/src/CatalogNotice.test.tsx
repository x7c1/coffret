import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it } from 'vitest';

import { CatalogNotice } from './CatalogNotice';
import { RAN_OUT, RECONNECTING } from './reconnect';
import { catalogLine } from './refresh';

afterEach(cleanup);

// A Library behind because the permission ran out says two sentences, the
// catch-up's and the reconnect's, and each stands in a block of its own rather
// than running on into the other on one line.
it('puts the catch-up sentence and the ran-out sentence on lines of their own', () => {
  const said = catalogLine(
    {
      state: 'behind',
      stopped: {
        kind: 'storage',
        message: "the Library's Storage no longer accepts this device's grant",
        reason: 'unauthenticated',
        surfaced: null,
      },
    },
    true,
  );
  expect(said).not.toBeNull();

  render(
    <CatalogNotice
      said={said as string}
      offer={{ reconnect: null, said: null, refused: null, consentPage: null, ask: () => undefined }}
    />,
  );

  const catchUp = screen.getByText(said as string);
  const ranOut = screen.getByText(RAN_OUT);
  expect(catchUp.tagName).toBe('DIV');
  expect(ranOut.tagName).toBe('DIV');
  // The catch-up's sentence comes first, and the button is still offered.
  expect(catchUp.compareDocumentPosition(ranOut) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(screen.getByRole('button', { name: RECONNECTING })).toBeTruthy();
});

// Without a reconnect to offer, the notice is the one sentence.
it('says the catch-up sentence alone where there is no reconnect', () => {
  render(<CatalogNotice said="this device is catching up with the Library" offer={null} />);
  expect(screen.getByText('this device is catching up with the Library').tagName).toBe('DIV');
  expect(screen.queryByText(RAN_OUT)).toBeNull();
});
