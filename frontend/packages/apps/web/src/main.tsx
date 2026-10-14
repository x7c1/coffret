import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { Explorer } from './Explorer';
import { establish, shelf, takeTicket } from './session';

// The ticket comes out of the address before anything else happens, the
// rendering included (spec: LA-21), and is exchanged once, here, whoever then
// waits on the answer.
const ticket = takeTicket(window.location, window.history);
const shelved = shelf();
const opened = establish(ticket, shelved);

const root = document.getElementById('root');
if (!root) {
  throw new Error('missing #root element');
}
createRoot(root).render(
  <StrictMode>
    <Explorer opened={opened} shelved={shelved} />
  </StrictMode>,
);
