/** Keeps the pre-Canvas launcher progress surface visible until a session is usable. */
export function dismissStartupOverlay(): void {
  const overlay = document.getElementById('loading-overlay');
  if (overlay !== null) {
    overlay.classList.add('opacity-0', 'pointer-events-none');
  }
}
