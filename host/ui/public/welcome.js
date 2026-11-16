'use strict';
const status = document.getElementById('status');
const button = document.getElementById('constitute');
const heading = document.querySelector('h1');
const introduction = document.getElementById('introduction');
// Only the approved packaged page runs here. All text is rendered as text,
// and the backend never supplies credentials, target IDs or an execution permit.
function render(result) {
  button.disabled = !result.constitution_available;
  button.hidden = result.phase === 'association-present';
  heading.textContent = button.hidden ? 'Welcome back to MAP' : 'Welcome to MAP';
  introduction.textContent = button.hidden
    ? 'This installation has a recorded sovereign association.'
    : 'Your personal space is yours to constitute.';
  status.textContent = result.message;
}
async function refresh() {
  try {
    const result = await window.__TAURI__.core.invoke('welcome_status');
    render(result);
    if (['initializing', 'provisioning', 'checking-space'].includes(result.phase)) {
      window.setTimeout(refresh, 1000);
    }
  } catch {
    button.disabled = true;
    status.textContent = 'MAP could not verify this Welcome session. Personal setup is unavailable.';
  }
}
button.addEventListener('click', async () => {
  button.disabled = true;
  try {
    // No identity, key, target or bootstrap assertion is accepted from this page.
    await window.__TAURI__.core.invoke('constitute_personal_space');
    await refresh();
  } catch (error) {
    status.textContent = typeof error === 'string'
      ? error
      : 'MAP could not authorize personal space setup.';
    button.disabled = true;
  }
});
void refresh();
