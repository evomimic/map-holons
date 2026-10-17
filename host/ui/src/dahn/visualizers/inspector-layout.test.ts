import { expect, it } from 'vitest';
import { inspectorLayout } from '../../../../conductora/visualizer-src/inspector-layout.js';
import { initialInspectorPresentation, inspectorPresentation, selectInspectorView } from '../../../../conductora/visualizer-src/inspector-presentation.js';
it('shares the main region exclusively between Properties and Collection', () => {
  const grant = { width: 800, height: 640 };
  const properties = inspectorLayout(grant, 48, 'properties'), collection = inspectorLayout(grant, 48, 'collection');
  expect(properties.details).toBe(true); expect(properties.showRail).toBe(true);
  expect(collection.details).toBe(false); expect(collection.showRail).toBe(false);
  expect(collection.body).toBe(collection.heading);
  expect(collection.title + collection.subjectActions + collection.body + collection.collection).toBe(grant.height);
  expect(properties.title + properties.subjectActions + properties.body + properties.collection).toBe(grant.height);
});
it('remembers Collection across compression and dimensions-only delivery, resuming it on Inspect', () => {
  let state = selectInspectorView(initialInspectorPresentation(), 'collection');
  state = inspectorPresentation(state, 'relationship');
  expect(state).toEqual({ mode: 'relationship', remembered: 'collection' });
  expect(inspectorPresentation(state, undefined)).toBe(state);
  state = inspectorPresentation(state, 'compact');
  expect(inspectorPresentation(state, 'inspect')).toEqual({ mode: 'collection', remembered: 'collection' });
});
it.each(['relationship', 'compact'])('%s hides both full-content regions without introducing a combined mode', mode => {
  const layout = inspectorLayout({ width: 800, height: 640 }, 48, mode);
  expect(layout.details).toBe(false); expect(layout.collection).toBe(0);
  expect(layout.showRail).toBe(mode === 'relationship');
});
