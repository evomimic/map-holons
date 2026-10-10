import HolonInspector, { horizontalOverflow } from './holon-inspector.js';

/** Same Node contract with connections before detail fields in the expanded view. */
export default class ConnectionsFirstInspector extends HolonInspector {
  static compositionSlots = {
    propertyMap: 'ConnectionsFirstInspector.PropertyMapSlot',
    action: 'ConnectionsFirstInspector.ActionsSlot',
    collection: 'ConnectionsFirstInspector.CollectionsSlot',
  };
  createRelationshipLayout(host, controls) {
    return horizontalOverflow(host, controls, 'More relationships');
  }
  getVisualizerComposition() {
    return super.getVisualizerComposition().map(region => region.element === this.singleValueRail
      ? { ...region, label: 'Connections Bar' } : region);
  }
  adaptBudget() {
    super.adaptBudget();
    if (!this.body || !this.singleValueRail) return;
    if (!this.maximizedRegion && (!this.verticalState || this.verticalState === 'full-height')
      && (!this.horizontalState || this.horizontalState === 'full-width')) {
      this.body.style.gridTemplateColumns = 'minmax(0, 1fr)';
      this.body.style.gridTemplateRows = 'max-content max-content minmax(0, 1fr)';
      this.singleValueRail.style.gridColumn = '1';
      this.singleValueRail.style.gridRow = '2';
      this.propertyViewer.style.gridRow = '3';
    }
  }
  setContext(context) {
    super.setContext(context);
    this.dataset.visualizerId = 'connections-first-inspector';
  }
}
