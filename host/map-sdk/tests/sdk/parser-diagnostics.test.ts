import { beforeEach, expect, it, vi } from 'vitest';
import { DomainError, MapClient, MalformedResponseError, readParserDiagnostics } from '../../src';
import fixture from '../fixtures/response-err-loader-parsing.json';
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
let failure: any;
beforeEach(() => {
  failure = structuredClone(fixture.result.Err);
  invoke.mockReset();
  invoke.mockImplementation(async (_command, { request }) => ({
    request_id: request.request_id,
    result: 'Space' in request.command ? { Ok: { TransactionCreated: { tx_id: 41 } } } : { Err: failure },
  }));
});
it('exposes Rust-authored parser evidence through public preparation and real wire decoding', async () => {
  const transaction = await new MapClient().beginTransaction();
  const error = await transaction.prepareHolons({ files_to_load: [{ filename: 'broken.json', raw_contents: '{' }] }).catch(e => e);
  expect(error).toBeInstanceOf(DomainError);
  expect(error.message).toBe('broken.json: invalid import');
  expect(readParserDiagnostics(error)).toEqual([{
    filename: 'broken.json', kind: 'StructuralValidationFailure', message: 'invalid import',
    location: { line: 2, column: 3 }, sourceError: { InvalidParameter: 'expected holons' },
  }]);
  expect(invoke).toHaveBeenCalledTimes(2);
});
it('retains repeated messages and absent locations without requiring subject handles', async () => {
  const issue = failure.LoaderParsingError.issues[0];
  failure.LoaderParsingError.issues = [issue, { ...issue, filename: 'another.json', location: null, source_error: null }];
  const tx = await new MapClient().beginTransaction();
  const error = await tx.prepareHolons({ files_to_load: [] }).catch(e => e);
  expect(readParserDiagnostics(error)?.map(d => [d.filename, d.location])).toEqual([
    ['broken.json', { line: 2, column: 3 }], ['another.json', null],
  ]);
});
it.each(['kind', 'location', 'source_error'])('rejects malformed %s in parser evidence', async field => {
  failure.LoaderParsingError.issues[0][field] = 'invalid';
  const tx = await new MapClient().beginTransaction();
  await expect(tx.prepareHolons({ files_to_load: [] })).rejects.toBeInstanceOf(MalformedResponseError);
});
it('does not reclassify unrelated failures', () => {
  expect(readParserDiagnostics(new Error('network'))).toBeNull();
});
