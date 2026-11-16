'use strict';
const status = document.getElementById('status');
const button = document.getElementById('constitute');
// This page imports no materialized code, accepts no external content and receives no credentials.
async function initialize() {
  try {
    const result = await window.__TAURI__.core.invoke('welcome_status');
    button.disabled = !result.constitution_available;
    status.textContent = result.constitution_available
      ? 'You can now create your personal space.'
      : 'Personal space setup is not available in this build.';
  } catch {
    button.disabled = true;
    status.textContent = 'MAP could not verify this Welcome session.';
  }
}
button.addEventListener('click', async () => {
  button.disabled = true;
  try {
    await window.__TAURI__.core.invoke('constitute_personal_space');
  } catch {
    status.textContent = 'Personal space setup is not available in this build.';
  }
});
void initialize();
