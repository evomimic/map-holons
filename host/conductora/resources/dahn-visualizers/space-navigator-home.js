export default class SpaceNavigatorHomeElement extends HTMLElement {
  setContext(context) {
    this.dataset.visualizerId = 'space-navigator-home';
    this.dataset.activeHolonSpace = context.holon.versionedKey();
    this.replaceChildren(Object.assign(document.createElement('h1'), {
      textContent: 'Space Navigator',
    }));
  }
}
