import HolonInspector from './holon-inspector.js';

/** Same Node contract with connections before detail fields in the expanded view. */
export default class ConnectionsFirstInspector extends HolonInspector {
  static compositionSlots = {
    propertyMap: 'ConnectionsFirstInspector.PropertyMapSlot',
    action: 'ConnectionsFirstInspector.ActionsSlot',
    collection: 'ConnectionsFirstInspector.CollectionsSlot',
  };
  adaptBudget() {
    super.adaptBudget();
    if (!this.body || !this.singleValueRail) return;
    if ((!this.verticalState || this.verticalState === 'full-height')
      && (!this.horizontalState || this.horizontalState === 'full-width')) {
      this.body.style.gridTemplateColumns = 'minmax(0, 1fr)';
      this.body.style.gridTemplateRows = 'max-content minmax(0, 1fr)';
      this.singleValueRail.style.gridColumn = '1';
      this.singleValueRail.style.gridRow = '1';
      this.propertyViewer.style.gridRow = '2';
      this.singleValueRail.style.maxHeight = '120px';
    }
  }
  setContext(context) {
    super.setContext(context);
    this.dataset.visualizerId = 'connections-first-inspector';
  }
}
