import type { BaseValue, DescribedHolonCollection, HolonReference, MapTransaction } from '../deps';
import type { CollectionInteractionElement, VisualizerContext } from '../contracts/visualizers';
import type { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import { DahnHolonView } from '../map-adapter/dahn-holon-view';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import { bindVisualizerInformationControl } from './visualizer-information-control';

/** Value input is descriptor-governed; projected values need no fabricated member handle. */
type ValueElement = HTMLElement & { setContext(context: Partial<VisualizerContext> & {
  propertyPresentation: { propertyName: string; value: BaseValue | null; missingValueLabel?: string };
}): void };

/** Install column selection before Table projects a described collection. */
export function configureColumnValueVisualizers(
  element: CollectionInteractionElement, transaction: MapTransaction,
  subject: HolonReference | DescribedHolonCollection,
  selectedTable: HolonReference, materialized: MaterializedVisualizerRuntime,
  current: () => boolean, context?: Partial<Pick<VisualizerContext, 'theme' | 'canvas'>>,
): void {
  let slot: Promise<HolonReference> | undefined;
  const requireCurrent = () => { if (!current()) throw new Error('Column presentation was superseded.'); };
  element.setColumnValueVisualizerProvider?.(async (property, name) => {
    const propertyName = await property.propertyName();
    const valueSlot = await (slot ??= materialized.slot(selectedTable, 'value'));
    requireCurrent();
    if (!context?.theme) throw new Error('Column selection requires the active semantic Theme.');
    const selection = await transaction.selectValueVisualizer(property, selectedTable, valueSlot, context.theme.reference);
    requireCurrent();
    const implementation = await materialized.realize(selection.selected);
    requireCurrent();
    if (typeof implementation !== 'function' || !(implementation.prototype instanceof HTMLElement)
      || typeof (implementation.prototype as Partial<ValueElement>).setContext !== 'function') throw new Error('Selected column Value Visualizer is not a Value presentation.');
    const tag = defineCustomElementOnce('map-table-value', implementation as CustomElementConstructor);
    return {
      create: (value, member, missingValueLabel) => {
        const cell = document.createElement(tag) as ValueElement;
        const reference = member ?? ('elementType' in subject ? undefined : subject);
        cell.setContext({ ...context, ...(reference ? { target: { reference }, holon: new DahnHolonView(reference) } : {}),
          actions: [], propertyPresentation: { propertyName, value, missingValueLabel } });
        return cell;
      },
      bindInformation: heading => bindVisualizerInformationControl(heading, transaction, subject, selectedTable,
        valueSlot, selection.selected, 'end', `Column: ${name} (all rows)`),
    };
  }, () => transaction.getSavedPropertyDescriptorByBaseKey('Key.PropertyType'));
}
