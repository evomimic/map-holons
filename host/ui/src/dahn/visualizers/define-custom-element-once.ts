export function defineCustomElementOnce(
  tagName: string,
  constructor: CustomElementConstructor,
): void {
  if (customElements.get(tagName) === undefined) {
    customElements.define(tagName, constructor);
  }
}
