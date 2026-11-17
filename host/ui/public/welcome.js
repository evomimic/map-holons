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
let constitutionCommitted = false;
const introduction = document.getElementById('introduction');
const activity = document.getElementById('activity');
const activityStage = document.getElementById('activity-stage');
const activityElapsed = document.getElementById('activity-elapsed');
let activityTimer = null;
let activityStarted = 0;
let activityStageStarted = 0;
let activityLabel = null;
const stageLabels = {
  'initializing-host': 'Starting MAP',
  'activating-holochain-app': 'Opening local storage',
  'opening-space': 'Opening your saved space',
  'preparing-core-schema': 'Preparing the foundation',
  'loading-core-schema': 'Loading the foundation',
  'verifying-core-schema': 'Verifying the foundation',
  'activating-base-packages': 'Preparing required packages',
  'loading-space-definitions': 'Loading required packages',
  'verifying-space-definitions': 'Verifying packaged definitions',
};
function elapsedSince(start) {
  const seconds = Math.max(0, Math.floor((Date.now() - start) / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}
function updateElapsed() {
  activityElapsed.textContent = `This step: ${elapsedSince(activityStageStarted)} · Total: ${elapsedSince(activityStarted)}`;
}
function showActivity(label) {
  if (activityTimer === null) {
    activityStarted = Date.now();
    activityLabel = null;
    activityTimer = window.setInterval(updateElapsed, 1000);
  }
  if (activityLabel !== label) {
    activityStageStarted = Date.now();
    activityLabel = label;
  }
  activity.hidden = false;
  activityStage.textContent = label;
  updateElapsed();
}
function stopActivity() {
  if (activityTimer !== null) window.clearInterval(activityTimer);
  activityTimer = null;
  activity.hidden = true;
}
// Only the approved packaged page runs here. All text is rendered as text,
// and the backend never supplies credentials, target IDs or an execution permit.
function render(result) {
  if (['initializing', 'provisioning', 'checking-space'].includes(result.phase)) {
    showActivity(stageLabels[result.stage] || (result.phase === 'provisioning' ? 'Preparing your space' : 'Checking your space'));
  } else if (constitutionCommitted && result.phase === 'foundation-present') {
    showActivity('Confirming your personal space');
  } else {
    stopActivity();
  }
  available = !constitutionCommitted && result.constitution_available === true && result.phase === 'foundation-present';
  button.disabled = !available || submitting;
  names.hidden = !available;
  names.disabled = !available || submitting;
  button.hidden = result.phase === 'association-present';
  heading.textContent = button.hidden ? 'Welcome back to MAP' : 'Welcome to MAP';
  introduction.textContent = button.hidden
    ? 'This installation has a recorded sovereign association.'
    : 'Your personal space is yours to constitute.';
  status.textContent = constitutionCommitted && result.phase === 'foundation-present'
    ? 'Your personal space was saved. MAP is confirming its recorded association.'
    : result.message;
}
async function refresh() {
  try {
    const result = await window.__TAURI__.core.invoke('welcome_status');
    render(result);
    if (['initializing', 'provisioning', 'checking-space'].includes(result.phase)
      || (constitutionCommitted && result.phase === 'foundation-present')) {
      window.setTimeout(refresh, 1000);
    }
  } catch {
    stopActivity();
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
  status.textContent = 'Creating your personal space…';
  showActivity('Creating your personal space');
  try {
    // Only names are sent; the backend selects the Space, author and principal.
    const outcome = await window.__TAURI__.core.invoke('constitute_personal_space', { input });
    if (outcome === 'complete') {
      constitutionCommitted = true;
      available = false;
      await refresh();
    } else {
      stopActivity();
      available = false;
      status.textContent = outcome === 'rejected'
        ? 'Constitution was rejected. The transaction and findings are retained for correction and resubmission.'
        : 'Constitution is incomplete. Saved and staged work is retained for remediation and resubmission.';
    }
  } catch (error) {
    stopActivity();
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
showActivity('Starting MAP');
void refresh();
