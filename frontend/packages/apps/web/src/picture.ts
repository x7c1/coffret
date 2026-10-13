// Where on the reader's `<img>` the page itself is drawn.
//
// The element is stretched to the room the reader has and the page is fitted
// inside it (`object-fit: contain`), so the element's box is wider or taller
// than the page wherever the two shapes differ. The bands either side are the
// overlay to a person looking at the screen, and a click there is a click
// beside the page — which closes the reader — even though the element under
// the pointer is the image.
//
// Kept free of DOM and of React so it is unit testable.

/** A rectangle on the screen, as `getBoundingClientRect` gives it. */
export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

/**
 * Whether `x`, `y` falls on the page drawn into `box`, the page being
 * `natural` pixels in size before it was fitted.
 *
 * A page whose size is not known yet — not decoded, or decoded to nothing — is
 * taken to fill the box, so that a click on it never closes the reader by a
 * guess.
 */
export function onPicture(
  box: Box,
  natural: { width: number; height: number },
  x: number,
  y: number,
): boolean {
  let { left, top, width, height } = box;
  if (natural.width > 0 && natural.height > 0) {
    const scale = Math.min(box.width / natural.width, box.height / natural.height);
    width = natural.width * scale;
    height = natural.height * scale;
    left += (box.width - width) / 2;
    top += (box.height - height) / 2;
  }
  return x >= left && x <= left + width && y >= top && y <= top + height;
}
