const registeredTags = new WeakMap<CustomElementConstructor, string>();
let nextImplementationId = 0;

/** Registers the supplied implementation without reusing another selection's constructor. */
export function defineCustomElementOnce(
  tagName: string,
  constructor: CustomElementConstructor,
): string {
  const registered = registeredTags.get(constructor);
  if (registered !== undefined) return registered;

  let implementationTag = tagName;
  while (customElements.get(implementationTag) !== undefined) {
    if (customElements.get(implementationTag) === constructor) {
      registeredTags.set(constructor, implementationTag);
      return implementationTag;
    }
    implementationTag = `${tagName}-${++nextImplementationId}`;
  }
  customElements.define(implementationTag, constructor);
  registeredTags.set(constructor, implementationTag);
  return implementationTag;
}
