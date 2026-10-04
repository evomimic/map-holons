import '@angular/compiler';
import { TestBed } from '@angular/core/testing';
import { BrowserTestingModule, platformBrowserTesting } from '@angular/platform-browser/testing';
import { afterEach, beforeAll, expect, it, vi } from 'vitest';
import { SourceReviewComponent } from './source-review.component';

vi.mock('./import-schema', () => ({ loadImportValidator: async () => (text: string) => ({
  valid: text !== 'bad', diagnostics: text === 'bad' ? ['Invalid JSON syntax'] : [],
}) }));
beforeAll(() => TestBed.initTestEnvironment(BrowserTestingModule, platformBrowserTesting()));
afterEach(() => TestBed.resetTestingModule());

it('renders validity and diagnostics, removes blockers, and emits only the selected snapshot once', async () => {
  const fixture = TestBed.createComponent(SourceReviewComponent);
  fixture.componentRef.setInput('discovery', { sources: [
    { id: 'a', path: '/one/import.json', content: '{"holons":[]}' },
    { id: 'b', path: '/two/import.json', content: 'bad' },
  ], issues: [] });
  const submitted = vi.fn();
  fixture.componentInstance.submitted.subscribe(submitted);
  await fixture.whenStable();
  fixture.detectChanges();
  const host = fixture.nativeElement as HTMLElement;
  const buttons = () => Array.from(host.querySelectorAll('button'));
  expect(host.textContent).toContain('Invalid JSON syntax');
  expect(host.querySelectorAll('input')[1].checked).toBe(true);
  expect(buttons()[1].disabled).toBe(true);
  buttons()[0].click();
  fixture.detectChanges();
  expect(host.querySelectorAll('input')).toHaveLength(1);
  expect(buttons()[1].disabled).toBe(true);
  const input = host.querySelector('input')!;
  input.checked = true;
  input.dispatchEvent(new Event('change'));
  fixture.detectChanges();
  expect(buttons()[1].disabled).toBe(false);
  buttons()[1].click();
  buttons()[1].click();
  expect(submitted).toHaveBeenCalledTimes(1);
  expect(submitted.mock.calls[0][0].files_to_load).toEqual([{ filename: '/one/import.json', raw_contents: '{"holons":[]}' }]);
  fixture.destroy();
});

it('exposes labeled checkboxes, a focusable scroll region and cancellation without handoff', async () => {
  const fixture = TestBed.createComponent(SourceReviewComponent);
  fixture.componentRef.setInput('discovery', { sources: Array.from({ length: 100 }, (_, i) => ({
    id: String(i), path: `/long/${i}/import.json`, content: '{}',
  })), issues: [] });
  const submitted = vi.fn(), cancelled = vi.fn();
  fixture.componentInstance.submitted.subscribe(submitted);
  fixture.componentInstance.cancelled.subscribe(cancelled);
  await fixture.whenStable();
  fixture.detectChanges();
  const host = fixture.nativeElement as HTMLElement;
  expect(host.querySelectorAll('label input')).toHaveLength(100);
  expect(host.querySelector('[aria-label="Source files and diagnostics"]')?.getAttribute('tabindex')).toBe('0');
  const buttons = host.querySelectorAll<HTMLButtonElement>('footer button');
  expect(buttons).toHaveLength(3);
  buttons[2].click();
  expect(cancelled).toHaveBeenCalledTimes(1);
  expect(submitted).not.toHaveBeenCalled();
  fixture.destroy();
});
