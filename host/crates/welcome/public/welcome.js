'use strict';
const status = document.getElementById('status');
const newEnvironmentButton = document.getElementById('new-environment');
let creatingEnvironment = false;
let environmentBusy = false;
let environmentChangesAvailable = false;
let environmentListLoaded = false;
const environmentPanel = document.getElementById('saved-environments');
const environmentList = document.getElementById('environment-list');
const environmentStatus = document.getElementById('environment-status');
const environmentRefresh = document.getElementById('refresh-environments');
const button = document.getElementById('constitute');
const setupForm = document.getElementById('personal-setup');
const heading = document.querySelector('h1');
const names = document.getElementById('personal-name');
const firstName = document.getElementById('first-name');
const middleName = document.getElementById('middle-name');
const lastName = document.getElementById('last-name');
const navigatorButton = document.getElementById('open-navigator');
const comparison = document.getElementById('usual-way');
const activityMessage = document.getElementById('activity-message');
const preparationProgress = document.getElementById('preparation-progress');
const attention = document.getElementById('attention');
const recoveryMessage = document.getElementById('recovery-message');
const spaceOptions = document.getElementById('space-options');
const startupDetails = document.getElementById('startup-details');
let lastStatus = null;
let newSpaceStarted = null;
const invitationReadingTime = 15000;
let localFailure = false;
let openingNavigator = false;
let navigatorAvailable = false;
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
  renderStory();
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
function text(element, value) {
  if (element.textContent !== value) element.textContent = value;
}
// Story screens describe observed lifecycle state. The comparison card is static
// illustration; it contains no account, credentials, or acceptance controls.
function renderStory() {
  const result = lastStatus;
  let view = 'discovery';
  if (localFailure || result?.phase === 'blocked') view = result?.incompatibility && !localFailure ? 'recovery' : 'problem';
  else if (result?.phase === 'association-present') view = constitutionCommitted ? 'ready' : 'returning';
  else if (constitutionCommitted && result?.phase === 'foundation-present') view = 'confirming';
  else if (result?.phase === 'foundation-present') view = result.constitution_available === true ? 'naming' : 'problem';
  else if (newSpaceStarted !== null) {
    const finishing = ['verifying-core-schema', 'activating-base-packages', 'loading-space-definitions', 'verifying-space-definitions'].includes(result?.stage);
    // Background setup continues; fast stage changes must not flash the comparison.
    // Ready and problem states above take precedence over this reading interval.
    view = Date.now() - newSpaceStarted < invitationReadingTime ? 'invitation' : finishing ? 'finalizing' : 'privacy';
  }
  const stories = {
    discovery: ['A space\nof your own.', 'Your data. On your devices. Under your control. Always.', 'Looking for your personal space…'],
    invitation: ['It all starts with\nan invitation.', 'Instead, MAP provisions a private, sovereign space for you…', 'Preparing your space…'],
    privacy: ['No account in\nsomeone else’s\ncloud.', 'MAP is preparing a private space for you, on your device.', 'Preparing your space…'],
    finalizing: ['Your data.\nOn your devices.\nUnder your control.\nAlways.', 'Your space is yours to steward.', 'Finalizing your space…'],
    naming: ['Your space is ready.\nMake yourself at home.', '', ''],
    confirming: ['Your data.\nOn your devices.\nUnder your control.\nAlways.', 'MAP is confirming your personal space.', 'Confirming your personal space…'],
    ready: ['You’re on\nthe MAP.', 'Your private space is ready, here on your devices, under your control.', ''],
    returning: ['Welcome\nback to MAP.', 'Your personal space is here, just as you left it.', ''],
    recovery: ['Your Space needs\na compatible environment.', 'Your existing Space is preserved.', ''],
    problem: ['Your space needs\nattention.', 'MAP could not finish verifying or opening your space.', ''],
  };
  const [title, copy, progress] = stories[view];
  document.body.dataset.view = view;
  text(heading, title);
  text(introduction, copy);
  introduction.hidden = !copy;
  comparison.hidden = view !== 'invitation';
  preparationProgress.hidden = !['invitation', 'privacy', 'finalizing', 'confirming'].includes(view) && !submitting;
  text(activityMessage, openingNavigator ? 'Entering your space…' : creatingEnvironment ? 'Preparing a fresh environment…' : submitting ? 'Making this space yours…' : progress);
  attention.hidden = !['recovery', 'problem'].includes(view);
  text(recoveryMessage, view === 'recovery'
    ? 'This version of MAP cannot open this environment. You can create a fresh environment or select another saved environment. Your previous environment and its data are kept.'
    : 'Your existing data has been retained. Review the setup details below for the next step.');
}
// Only the approved packaged page runs here. All text is rendered as text,
// and the backend never supplies credentials, target IDs or an execution permit.
function render(result) {
  lastStatus = result;
  localFailure = false;
  if (newSpaceStarted === null && (result.preparing_new_space === true || result.stage === 'loading-core-schema')) newSpaceStarted = Date.now();
  environmentPanel.hidden = result.environment_management_available !== true;
  environmentChangesAvailable = !environmentPanel.hidden && ['foundation-present', 'association-present', 'blocked'].includes(result.phase);
  newEnvironmentButton.hidden = result.new_environment_available !== true;
  newEnvironmentButton.disabled = creatingEnvironment || openingNavigator;
  newEnvironmentButton.textContent = result.navigator_available === true
    ? 'Reset and restart my Space' : 'Create a new environment (keep existing)';
  document.getElementById('reset-description').hidden = newEnvironmentButton.hidden;
  spaceOptions.hidden = newEnvironmentButton.hidden;
  if (result.incompatibility) spaceOptions.open = true;
  if (['initializing', 'provisioning', 'checking-space'].includes(result.phase)) {
    showActivity(stageLabels[result.stage] || (result.phase === 'provisioning' ? 'Preparing your space' : 'Checking your space'));
  } else if (constitutionCommitted && result.phase === 'foundation-present') {
    showActivity('Confirming your personal space');
  } else {
    stopActivity();
  }
  navigatorAvailable = result.navigator_available === true;
  navigatorButton.hidden = !navigatorAvailable;
  navigatorButton.disabled = openingNavigator || creatingEnvironment;
  available = !constitutionCommitted && result.constitution_available === true && result.phase === 'foundation-present';
  button.disabled = !available || submitting;
  names.hidden = !available;
  names.disabled = !available || submitting;
  button.hidden = !available;
  status.textContent = constitutionCommitted && result.phase === 'foundation-present'
    ? 'Your personal space was saved. MAP is confirming its recorded association.'
    : result.incompatibility ? `This environment is incompatible: ${result.incompatibility}. You can create a separate environment. Your existing environment will be preserved.` : result.message;
  renderStory();
  updateEnvironmentControls();
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
    localFailure = true;
    startupDetails.open = true;
    renderStory();
    available = false;
    names.hidden = true;
    button.hidden = true;
    names.disabled = true;
    button.disabled = true;
    status.textContent = 'MAP could not verify this Welcome session. Personal setup is unavailable.';
  }
}
async function openNavigator() {
  if (!navigatorAvailable || openingNavigator || creatingEnvironment || environmentBusy) return;
  openingNavigator = true;
  updateEnvironmentControls();
  navigatorButton.disabled = true;
  newEnvironmentButton.disabled = true;
  status.textContent = 'Opening Space Navigator…';
  showActivity('Opening Space Navigator');
  try {
    await window.__TAURI__.core.invoke('open_space_navigator');
    stopActivity();
  } catch (error) {
    stopActivity();
    status.textContent = typeof error === 'string' ? error : 'MAP could not open Space Navigator. Your personal space is retained.';
    navigatorButton.hidden = false;
    startupDetails.open = true;
  } finally {
    openingNavigator = false;
    updateEnvironmentControls();
    navigatorButton.disabled = false;
    newEnvironmentButton.disabled = false;
  }
}
newEnvironmentButton.addEventListener('click', async () => {
  if (creatingEnvironment || openingNavigator || environmentBusy || newEnvironmentButton.hidden) return;
  creatingEnvironment = true;
  updateEnvironmentControls();
  newEnvironmentButton.disabled = true;
  navigatorButton.disabled = true;
  status.textContent = 'Starting a fresh Space and restarting MAP. Your current Space is kept.';
  showActivity('Restarting your Space');
  try {
    await window.__TAURI__.core.invoke('create_new_environment');
  } catch (error) {
    stopActivity();
    status.textContent = typeof error === 'string' ? error : 'MAP could not select a new environment. Existing data has been retained.';
    creatingEnvironment = false;
    startupDetails.open = true;
    updateEnvironmentControls();
    newEnvironmentButton.disabled = false;
    navigatorButton.disabled = !navigatorAvailable;
  }
});
navigatorButton.addEventListener('click', () => { void openNavigator(); });
setupForm.addEventListener('submit', async event => {
  event.preventDefault();
  if (!available || submitting || environmentBusy || !lastName.reportValidity()) return;
  const input = { lastName: lastName.value };
  if (firstName.value !== '') input.firstName = firstName.value;
  if (middleName.value !== '') input.middleName = middleName.value;
  submitting = true;
  updateEnvironmentControls();
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
      localFailure = true;
      renderStory();
      names.hidden = true;
      button.hidden = true;
      startupDetails.open = true;
      available = false;
      status.textContent = outcome === 'rejected'
        ? 'Constitution was rejected. The transaction and findings are retained for correction and resubmission.'
        : 'Constitution is incomplete. Saved and staged work is retained for remediation and resubmission.';
    }
  } catch (error) {
    stopActivity();
    localFailure = true;
    renderStory();
    names.hidden = true;
    button.hidden = true;
    startupDetails.open = true;
    status.textContent = typeof error === 'string'
      ? error
      : 'MAP could not authorize personal space setup.';
    available = false;
    button.disabled = true;
  } finally {
    submitting = false;
    updateEnvironmentControls();
    button.disabled = !available;
    names.disabled = !available;
  }
});
function updateEnvironmentControls() {
  const busy = environmentBusy || openingNavigator || creatingEnvironment || submitting;
  navigatorButton.disabled = !navigatorAvailable || busy;
  newEnvironmentButton.disabled = busy;
  button.disabled = !available || busy;
  names.disabled = !available || busy;
  environmentRefresh.disabled = busy;
  for (const control of environmentList.querySelectorAll('button')) control.disabled = busy || localFailure || !environmentChangesAvailable;
}
function environmentSize(bytes) {
  if (typeof bytes !== 'number') return 'Size unavailable';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let unit = 0;
  while (bytes >= 1024 && unit < units.length - 1) { bytes /= 1024; unit++; }
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(bytes)} ${units[unit]}`;
}
async function loadEnvironments() {
  if (environmentBusy || openingNavigator || creatingEnvironment || submitting) return;
  environmentBusy = true; updateEnvironmentControls();
  environmentStatus.textContent = 'Reading saved environments…';
  try {
    const environments = await window.__TAURI__.core.invoke('saved_environments');
    environmentList.replaceChildren();
    for (const item of environments) {
      const card = document.createElement('section'); card.className = 'saved-environment';
      card.dataset.environmentId = item.id;
      const title = document.createElement('h2');
      const created = item.created_seconds ? new Date(item.created_seconds * 1000).toLocaleString() : 'Creation date unavailable';
      const label = item.id === 'local-constitution' ? 'Original environment' : `Environment · ${created}`;
      title.textContent = item.active ? `${label} · Active` : label;
      const info = document.createElement('p'); info.textContent = `${environmentSize(item.size_bytes)}${item.active ? ' · Currently selected' : ' · Compatibility checked on restart'}`;
      card.append(title, info);
      if (!item.active) {
        const actions = document.createElement('div'); actions.className = 'environment-actions';
        const activate = document.createElement('button'); activate.type = 'button'; activate.textContent = 'Reactivate and restart';
        activate.addEventListener('click', () => { void changeEnvironment('activate_saved_environment', item.id); });
        const trash = document.createElement('button'); trash.type = 'button'; trash.textContent = 'Move to Trash';
        trash.addEventListener('click', () => {
          if (environmentBusy || !environmentChangesAvailable || card.querySelector('.environment-confirmation')) return;
          const confirm = document.createElement('div'); confirm.className = 'environment-confirmation';
          const description = document.createElement('p'); description.textContent = `Move ${label} and its identity and Space data to Trash? It can be recovered until Trash is emptied.`;
          const accept = document.createElement('button'); accept.type = 'button'; accept.textContent = 'Confirm move to Trash';
          accept.addEventListener('click', () => { void changeEnvironment('trash_saved_environment', item.id); });
          const cancel = document.createElement('button'); cancel.type = 'button'; cancel.textContent = 'Cancel';
          cancel.addEventListener('click', () => { confirm.remove(); trash.focus(); });
          confirm.append(description, accept, cancel); card.append(confirm); accept.focus();
        });
        actions.append(activate, trash); card.append(actions);
      }
      environmentList.append(card);
    }
    environmentStatus.textContent = environments.length ? `${environments.length} saved environment${environments.length === 1 ? '' : 's'}. The active environment cannot be removed.` : 'No saved environments found.';
    environmentListLoaded = true;
  } catch (error) {
    environmentStatus.textContent = typeof error === 'string' ? error : 'MAP could not read saved environments. Nothing was changed.';
  } finally {
    environmentBusy = false; updateEnvironmentControls();
  }
}
async function changeEnvironment(command, id) {
  if (environmentBusy || !environmentChangesAvailable || openingNavigator || creatingEnvironment || submitting) return;
  environmentBusy = true; updateEnvironmentControls();
  const restarting = command === 'activate_saved_environment';
  environmentStatus.textContent = restarting ? 'Reactivating the selected environment and restarting MAP…' : 'Moving the inactive environment to Trash…';
  try {
    await window.__TAURI__.core.invoke(command, { id });
    if (restarting) return;
    environmentBusy = false;
    await loadEnvironments();
  } catch (error) {
    environmentStatus.textContent = typeof error === 'string' ? error : 'MAP could not change the environment. Retry or refresh the list.';
    environmentBusy = false; updateEnvironmentControls();
  }
}
environmentPanel.addEventListener('toggle', () => { if (environmentPanel.open && !environmentListLoaded) void loadEnvironments(); });
environmentRefresh.addEventListener('click', () => { void loadEnvironments(); });
showActivity('Starting MAP');
void refresh();
