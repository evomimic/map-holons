import { DomainError, extractNumber, extractString, type ReadableHolon } from '../../../dahn/deps/map-sdk';

export interface LoaderErrorView {
  filename: string;
  startUtf8ByteOffset: string;
  loaderHolonKey: string;
  errorType: string;
  errorMessage: string;
}

export interface LoaderResultView {
  holonsStaged: string;
  holonsCommitted: string;
  errorCount: string;
  validationViolationCount: string;
  danceSummary: string;
  linksCreated: string;
  loadCommitStatus: string;
  loadErrors: LoaderErrorView[];
  readFailures: string[];
  outcome: string;
}

const UNAVAILABLE = 'Not available';

/** Keep the domain payload: Error.message alone often contains only its variant. */
export function loaderFailureDetail(error: unknown): string {
  if (error instanceof DomainError) {
    return `${error.message}: ${typeof error.payload === 'string' ? error.payload : JSON.stringify(error.payload)}`;
  }
  return error instanceof Error ? error.message : String(error);
}

/** Read fields independently so a presentation failure cannot erase usable evidence. */
export async function presentLoaderResult(holon: ReadableHolon): Promise<LoaderResultView> {
  const readFailures: string[] = [];
  async function read(source: ReadableHolon, name: string, integer = false): Promise<string> {
    try {
      const value = await source.propertyValue(name);
      if (value === null) return UNAVAILABLE;
      if (!integer) return extractString(value);
      const number = extractNumber(value);
      if (!Number.isSafeInteger(number) || number < 0) throw new Error('Expected a nonnegative integer');
      return String(number);
    } catch (error) {
      readFailures.push(`${name}: ${loaderFailureDetail(error)}`);
      return UNAVAILABLE;
    }
  }
  const [holonsStaged, holonsCommitted, errorCount, validationViolationCount,
    danceSummary, linksCreated, loadCommitStatus, totalLoaderHolons] = await Promise.all([
    read(holon, 'HolonsStaged', true), read(holon, 'HolonsCommitted', true),
    read(holon, 'ErrorCount', true), read(holon, 'ValidationViolationCount', true),
    read(holon, 'DanceSummary'), read(holon, 'LinksCreated', true),
    read(holon, 'LoadCommitStatus'), read(holon, 'TotalLoaderHolons', true),
  ]);
  let loadErrors: LoaderErrorView[] = [];
  let diagnosticsReadable = true;
  try {
    const collection = await holon.relatedHolons('HasLoadError');
    loadErrors = await Promise.all(collection.members.map(async source => ({
      filename: await read(source, 'Filename'),
      startUtf8ByteOffset: await read(source, 'StartUtf8ByteOffset', true),
      loaderHolonKey: await read(source, 'LoaderHolonKey'),
      errorType: await read(source, 'ErrorType'),
      errorMessage: await read(source, 'ErrorMessage'),
    })));
  } catch (error) {
    diagnosticsReadable = false;
    readFailures.push(`HasLoadError: ${loaderFailureDetail(error)}`);
  }

  let outcome: string;
  switch (loadCommitStatus) {
    case 'Complete': outcome = 'Load complete'; break;
    case 'Rejected': outcome = 'Load rejected'; break;
    case 'Incomplete':
      outcome = holonsCommitted === UNAVAILABLE ? 'Load incomplete — saved count is unavailable'
        : Number(holonsCommitted) > 0 ? 'Load incomplete — some holons were saved'
        : 'Load incomplete — no holons were saved';
      break;
    case 'Skipped':
      outcome = totalLoaderHolons === '0' && errorCount === '0' && validationViolationCount === '0'
        && diagnosticsReadable && loadErrors.length === 0 ? 'Nothing to load' : 'Load skipped';
      break;
    default:
      outcome = 'Load could not be completed';
      readFailures.push(`LoadCommitStatus is unavailable or unrecognized: ${loadCommitStatus}`);
  }
  return { holonsStaged, holonsCommitted, errorCount, validationViolationCount,
    danceSummary, linksCreated, loadCommitStatus, loadErrors, readFailures, outcome };
}
