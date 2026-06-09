import { extractNumber, extractString, type ReadableHolon } from '../../../dahn/deps/map-sdk';

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
  danceSummary: string;
  linksCreated: string;
  loadCommitStatus: string;
  loadErrors: LoaderErrorView[];
}

const CORE_PROPERTY_NAMES = {
  holonsStaged: 'HolonsStaged',
  holonsCommitted: 'HolonsCommitted',
  errorCount: 'ErrorCount',
  danceSummary: 'DanceSummary',
  linksCreated: 'LinksCreated',
  loadCommitStatus: 'LoadCommitStatus',
} as const;

const LOAD_ERROR_PROPERTY_NAMES = {
  filename: 'Filename',
  startUtf8ByteOffset: 'StartUtf8ByteOffset',
  loaderHolonKey: 'LoaderHolonKey',
  errorType: 'ErrorType',
  errorMessage: 'ErrorMessage',
} as const;

export async function presentLoaderResult(
  loaderHolon: ReadableHolon,
): Promise<LoaderResultView> {
  const [
    holonsStaged,
    holonsCommitted,
    errorCount,
    danceSummary,
    linksCreated,
    loadCommitStatus,
    loadErrors,
  ] = await Promise.all([
    readIntegerProperty(loaderHolon, CORE_PROPERTY_NAMES.holonsStaged),
    readIntegerProperty(loaderHolon, CORE_PROPERTY_NAMES.holonsCommitted),
    readIntegerProperty(loaderHolon, CORE_PROPERTY_NAMES.errorCount),
    readStringProperty(loaderHolon, CORE_PROPERTY_NAMES.danceSummary),
    readIntegerProperty(loaderHolon, CORE_PROPERTY_NAMES.linksCreated),
    readStringProperty(loaderHolon, CORE_PROPERTY_NAMES.loadCommitStatus),
    readLoadErrors(loaderHolon),
  ]);

  return {
    holonsStaged,
    holonsCommitted,
    errorCount,
    danceSummary,
    linksCreated,
    loadCommitStatus,
    loadErrors,
  };
}

async function readIntegerProperty(
  holon: ReadableHolon,
  propertyName: string,
): Promise<string> {
  const value = await holon.propertyValue(propertyName);
  if (value === null) {
    return 'n/a';
  }

  try {
    return String(extractNumber(value));
  } catch {
    return JSON.stringify(value);
  }
}

async function readStringProperty(
  holon: ReadableHolon,
  propertyName: string,
): Promise<string> {
  const value = await holon.propertyValue(propertyName);
  if (value === null) {
    return 'n/a';
  }

  try {
    return extractString(value);
  } catch {
    return JSON.stringify(value);
  }
}

async function readLoadErrors(
  loaderHolon: ReadableHolon,
): Promise<LoaderErrorView[]> {
  const errorCollection = await loaderHolon.relatedHolons('HasLoadError');

  return Promise.all(
    errorCollection.members.map(async (errorHolon: ReadableHolon) => ({
      filename: await readStringProperty(errorHolon, LOAD_ERROR_PROPERTY_NAMES.filename),
      startUtf8ByteOffset: await readIntegerProperty(
        errorHolon,
        LOAD_ERROR_PROPERTY_NAMES.startUtf8ByteOffset,
      ),
      loaderHolonKey: await readStringProperty(
        errorHolon,
        LOAD_ERROR_PROPERTY_NAMES.loaderHolonKey,
      ),
      errorType: await readStringProperty(errorHolon, LOAD_ERROR_PROPERTY_NAMES.errorType),
      errorMessage: await readStringProperty(
        errorHolon,
        LOAD_ERROR_PROPERTY_NAMES.errorMessage,
      ),
    })),
  );
}
