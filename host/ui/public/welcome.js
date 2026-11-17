'use strict';
const status = document.getElementById('status');
const button = document.getElementById('constitute');
const heading = document.querySelector('h1');
const names = document.getElementById('personal-name');
const firstName = document.getElementById('first-name');
const middleName = document.getElementById('middle-name');
const lastName = document.getElementById('last-name');
let available = false;
let submitting = false;
const introduction = document.getElementById('introduction');
// Only the approved packaged page runs here. All text is rendered as text,
// and the backend never supplies credentials, target IDs or an execution permit.
function render(result) {
  available = result.constitution_available === true && result.phase === 'foundation-present';
  button.disabled = !available || submitting;
  names.hidden = !available;
  names.disabled = !available || submitting;
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
    available = false;
    names.disabled = true;
    button.disabled = true;
    status.textContent = 'MAP could not verify this Welcome session. Personal setup is unavailable.';
  }
}
button.addEventListener('click', async () => {
  if (!available || submitting || !lastName.reportValidity()) return;
  const input = { lastName: lastName.value };
  if (firstName.value !== '') input.firstName = firstName.value;
  if (middleName.value !== '') input.middleName = middleName.value;
  submitting = true;
  button.disabled = true;
  names.disabled = true;
  try {
    // Only names are sent; the backend selects the Space, author and principal.
    await window.__TAURI__.core.invoke('constitute_personal_space', { input });
    await refresh();
  } catch (error) {
    status.textContent = typeof error === 'string'
      ? error
      : 'MAP could not authorize personal space setup.';
    available = false;
    button.disabled = true;
  } finally {
    submitting = false;
    button.disabled = !available;
    names.disabled = !available;
  }
});
void refresh();
