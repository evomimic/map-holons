/** Real-layout checks for the bounded production composition in the acceptance page. */
export async function checkValuePresentationLayout(host) {
  const settle = async () => { for (let i = 0; i < 4; i++) await new Promise(requestAnimationFrame); };
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const pane = host.querySelector('[data-holon-inspector-property-viewer]');
  const properties = host.querySelector('[data-dahn-properties]');
  const list = host.querySelector('[data-dahn-properties-list]');
  const button = properties.querySelector('button');
  const footer = properties.querySelector('footer');
  const rows = [...properties.querySelectorAll('[data-dahn-property-slot]')];
  const collections = host.querySelector('[data-holon-inspector-collection-tab-bar]');
  const results = [];
  const prefix = () => {
    const visible = rows.filter(row => getComputedStyle(row).visibility === 'visible');
    assert(visible.every((row, index) => row === rows[index]), 'Visible rows must form the supplied prefix');
    const bounds = list.getBoundingClientRect();
    assert(visible.every(row => row.getBoundingClientRect().bottom <= bounds.bottom + 0.5), 'Collapsed rows must fit completely');
    assert(pane.getBoundingClientRect().bottom <= collections.getBoundingClientRect().top, 'Properties must not overlap Collections');
    return visible.length;
  };
  try {
    host.style.width = '1100px'; host.style.height = '850px'; await settle();
    const count = prefix();
    assert(count > 0 && count < rows.length, 'Fixture must exercise a nonempty truncated prefix');
    assert(button.textContent.includes(`Show ${rows.length - count} more`), 'Disclosure must report hidden count');
    assert(rows.slice(count).every(row => row.inert && row.getAttribute('aria-hidden') === 'true'), 'Hidden rows must be inaccessible');
    results.push('collapsed ordered prefix and hidden count');
    const paneBounds = pane.getBoundingClientRect();
    const footerTop = footer.getBoundingClientRect().top;
    button.focus(); button.click(); await settle();
    assert(button.getAttribute('aria-expanded') === 'true', 'Expansion state');
    assert(pane.getBoundingClientRect().height === paneBounds.height, 'Expansion must preserve pane height');
    assert(footer.getBoundingClientRect().top === footerTop, 'Disclosure must stay stationary');
    assert(getComputedStyle(list).overflowY === 'auto' && list.scrollHeight > list.clientHeight, 'Only the list becomes scrollable');
    list.scrollTop = list.scrollHeight; await settle();
    assert(list.scrollTop > 0 && footer.getBoundingClientRect().top === footerTop, 'Footer must remain visible while scrolling');
    button.click(); await settle();
    assert(list.scrollTop === 0 && document.activeElement === button, 'Collapse resets scrolling and retains focus');
    prefix(); results.push('expand, scroll, and collapse within unchanged region');
    host.style.height = '2200px'; await settle();
    assert(prefix() === rows.length && footer.inert, 'All-fitting list omits disclosure');
    host.style.height = '650px'; host.style.width = '760px'; await settle();
    prefix(); results.push('height and width changes recalculate fitting');
    const firstChild = rows[0].firstElementChild;
    const originalHeight = firstChild.style.height;
    firstChild.style.height = '400px'; await settle();
    assert(prefix() === 0, 'An oversized first row cannot be skipped for a later row');
    assert(button.textContent.includes(`Show ${rows.length} more`), 'No-row-fit disclosure count');
    firstChild.style.height = originalHeight; await settle();
    prefix(); results.push('child-size changes and no complete row fits');
    properties.style.height = '42px'; await settle();
    assert(footer.getBoundingClientRect().bottom <= properties.getBoundingClientRect().bottom + 0.5, 'Tiny allocations prioritize the disclosure when it fits');
    assert(rows.every(row => row.inert), 'Tiny allocation must not expose partial rows');
    properties.style.height = '100%'; await settle();
    results.push('tiny allocation keeps disclosure contained');
    return results;
  } finally {
    host.style.removeProperty('height'); host.style.removeProperty('width'); await settle();
  }
}
