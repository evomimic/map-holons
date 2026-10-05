import { DomainError } from '../internal/errors';
import { isLoaderParsingFailureWire } from '../internal/wire-types/references';
import type { HolonError } from './types';

/** Parser evidence has no staged subject and remains usable without a response holon. */
export interface ParserDiagnostic {
  filename: string;
  kind: 'IoFailure' | 'JsonDecodingFailure' | 'StructuralValidationFailure' | 'HolonConstructionFailure';
  message: string;
  /** Original-file coordinates: one-based line and UTF-8 byte column; column zero can occur at EOF. */
  location: { line: number; column: number } | null;
  sourceError: HolonError | null;
}

/** Return structured parser evidence, or null when this is another kind of failure. */
export function readParserDiagnostics(error: unknown): ParserDiagnostic[] | null {
  if (!(error instanceof DomainError) || error.variant !== 'LoaderParsingError') return null;
  if (!isLoaderParsingFailureWire(error.payload)) throw new TypeError('Malformed loader parsing diagnostics');
  return error.payload.issues.map(issue => ({
    filename: issue.filename, kind: issue.kind, message: issue.message,
    location: issue.location ? { ...issue.location } : null, sourceError: issue.source_error,
  }));
}
