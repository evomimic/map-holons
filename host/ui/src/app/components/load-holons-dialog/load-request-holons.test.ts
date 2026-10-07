import { expect, it } from 'vitest';
import { createLoadRequest } from './load-request-holons';

it('captures exact source snapshots in a typed request before preparation exists', async () => {
  const created: any[] = [];
  const transaction = {
    getSavedHolonByBaseKey: async (key: string) => ({ key }),
    newHolon: async () => {
      const properties: Record<string, unknown> = {}, relationships: Record<string, unknown> = {};
      const holon = { properties, relationships, descriptor: undefined as unknown,
        withDescriptor: async (type: unknown) => { holon.descriptor = type; },
        withPropertyValue: async (name: string, value: unknown) => { properties[name] = value; },
        addRelatedHolons: async (name: string, members: unknown[]) => { relationships[name] = members; } };
      created.push(holon); return holon;
    },
  };
  const contents = { files_to_load: [{ filename: '/a/data.json', raw_contents: ' {"a": 1}\n' }, { filename: '/b/data.json', raw_contents: '{}' }] };
  const request = await createLoadRequest(transaction as never, contents);
  contents.files_to_load[0].raw_contents = 'changed';
  expect(request.reference).toBe(created[0]);
  expect(created[0].descriptor).toEqual({ key: 'LoadRequest.Projection' });
  expect(created[0].relationships.SubmittedSources).toEqual([created[1]]);
  expect(created[1].relationships.Sources).toEqual([created[2], created[3]]);
  expect(created[2].properties).toEqual({ Filename: { StringValue: '/a/data.json' }, SourceContents: { StringValue: ' {"a": 1}\n' } });
  expect(created[0].relationships.PreparedLoadSet).toBeUndefined();
  expect(created[0].relationships.LoadResponse).toBeUndefined();
  expect(created[0].properties.LoadCommitStatus).toBeUndefined();
});
