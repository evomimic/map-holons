#!/usr/bin/env python3
"""Predict VAL-C1a cohort findings over the canonical corpus, without a Holochain run.

TEMPORARY — delete this script in the final phase of issue #651.

This is a fast, independent oracle for the report-only conformance pass that
`tests/sweetests` performs through `holons_validation`. It reads the generated
loader JSON directly, so it answers "did the corpus change fix what I expected?"
in a couple of seconds instead of a full `npm run sweetest` cycle under Nix.

It is deliberately NOT wired into `npm run check` or `npm run test:unit`: the
Sweettest is the acceptance gate. This script exists so that a disagreement
between the two is visible early — if the Rust validator and this script report
different findings, one of them has a bug.

It reimplements just enough descriptor kernel to evaluate the three VAL-C1a
cohort checks:

  * DS-PROP-001  required-property presence, gated by EnforceMinimum
  * DS-PROP-003  no undescribed populated properties
  * native BaseValue-versus-ValueType kind compatibility

Semantics mirrored from the descriptor kernel (see
map-dev-docs/docs/core/type-system/descriptor-semantics-rules.md):

  * L(H) is the self-first Extends lineage.
  * InstanceProperties is Additive, so the effective contract is the
    ancestor-before-local union across L(D(H)).
  * EnforceMinimum(H, M) iff not IsAbstract(H) or UniversalDescriptorMember(M),
    where the universal set is the effective contract of MetaTypeDescriptor.HolonType.
  * A required property that declares a DefaultValue is filled by
    WritableHolon::populate_defaults() before validation, so its absence from the
    loader JSON is not a finding.

Usage:
    python3 scripts/predict-c1-conformance.py [--imports generated/json-imports] [-v]

Exits non-zero when any finding is predicted.
"""

from __future__ import annotations

import argparse
import collections
import json
import pathlib
import sys

META_TYPE_DESCRIPTOR = "MetaTypeDescriptor.HolonType"

# Value-type family roots, keyed by canonical schema key. The Rust validator
# resolves these by key once per validation run and then compares by reference
# identity; here the key comparison is the identity.
VALUE_FAMILY_ROOTS = {
    "StringValueType.ValueType": "String",
    "IntegerValueType.ValueType": "Integer",
    "BooleanValueType.ValueType": "Boolean",
    "BytesValueType.ValueType": "Bytes",
    "EnumValueType.ValueType": "Enum",
    "BaseValueValueType.ValueType": "AnyBaseValue",
    "ValueArrayValueType.ValueType": "ValueArray",
}


def load_corpus(imports_root: pathlib.Path) -> dict[str, dict]:
    """Indexes every holon in the generated loader JSON by its canonical key."""
    holons: dict[str, dict] = {}
    files = sorted(imports_root.glob("*/*.json"))
    if not files:
        sys.exit(f"no loader JSON found under {imports_root}")
    for path in files:
        package = path.relative_to(imports_root).as_posix()
        document = json.loads(path.read_text())
        for holon in document.get("holons", []):
            holon["_package"] = package
            holons[holon["key"]] = holon
    return holons


class Kernel:
    """The slice of descriptor-kernel semantics the C1a cohort depends on."""

    def __init__(self, holons: dict[str, dict]):
        self.holons = holons
        self._lineage_cache: dict[str, list[str]] = {}
        self._contract_cache: dict[str, list[str]] = {}
        self.universal_members = frozenset(self.effective_property_contract(META_TYPE_DESCRIPTOR))

    def targets(self, key: str, relationship: str) -> list[str]:
        for entry in self.holons[key].get("relationships", []):
            if entry["name"] == relationship:
                return [target["$ref"] for target in entry.get("target", [])]
        return []

    def lineage(self, key: str) -> list[str]:
        """L(H): self-first Extends chain, stopping on an unresolved ref or a cycle."""
        if key in self._lineage_cache:
            return self._lineage_cache[key]
        chain: list[str] = []
        seen: set[str] = set()
        current: str | None = key
        while current and current in self.holons and current not in seen:
            seen.add(current)
            chain.append(current)
            parents = self.targets(current, "Extends")
            current = parents[0] if parents else None
        self._lineage_cache[key] = chain
        return chain

    def effective_property_contract(self, descriptor_key: str) -> list[str]:
        """Ancestor-before-local union of InstanceProperties across the lineage."""
        if descriptor_key in self._contract_cache:
            return self._contract_cache[descriptor_key]
        refs: list[str] = []
        for ancestor in reversed(self.lineage(descriptor_key)):
            refs += self.targets(ancestor, "InstanceProperties")
        deduped = list(dict.fromkeys(refs))
        self._contract_cache[descriptor_key] = deduped
        return deduped

    def effective_property_value(self, descriptor_key: str, name: str):
        """Self-first walk for a descriptor-level property such as IsValueRequired."""
        for ancestor in self.lineage(descriptor_key):
            value = self.holons[ancestor].get("properties", {}).get(name)
            if value is not None:
                return value
        return None

    def is_abstract(self, holon: dict) -> bool:
        # IsAbstractType declares DefaultValue false, so an absent value is false.
        return bool(holon.get("properties", {}).get("IsAbstractType", False))

    def enforce_minimum(self, holon: dict, member_ref: str) -> bool:
        return not self.is_abstract(holon) or member_ref in self.universal_members

    def allows_additional_properties(self, descriptor_key: str) -> bool:
        value = self.effective_property_value(descriptor_key, "AllowsAdditionalProperties")
        # AllowsAdditionalProperties declares DefaultValue false.
        return bool(value) if value is not None else False

    def value_family(self, property_ref: str) -> str | None:
        """Classifies a property descriptor's selected ValueType into a native family."""
        selected = None
        for ancestor in self.lineage(property_ref):
            targets = self.targets(ancestor, "ValueType")
            if targets:
                selected = targets[0]
                break
        if selected is None:
            return None
        for ancestor in self.lineage(selected):
            if ancestor in VALUE_FAMILY_ROOTS:
                return VALUE_FAMILY_ROOTS[ancestor]
        return f"Unsupported({selected})"


def native_kind(value) -> str:
    if isinstance(value, bool):
        return "Boolean"
    if isinstance(value, int):
        return "Integer"
    if isinstance(value, str):
        # Enum tokens and bytes both serialize as strings in loader JSON, so a
        # string is only reported as a mismatch against a numeric or boolean family.
        return "String"
    return type(value).__name__


Finding = collections.namedtuple("Finding", "rule package holon detail")


def assess(kernel: Kernel) -> list[Finding]:
    findings: list[Finding] = []

    for key, holon in kernel.holons.items():
        descriptor_key = holon.get("type")
        package = holon["_package"]

        if not descriptor_key:
            findings.append(Finding("NoDescriptor", package, key, "holon declares no type"))
            continue
        if descriptor_key not in kernel.holons:
            findings.append(
                Finding("NoDescriptor", package, key, f"unresolved type {descriptor_key!r}")
            )
            continue

        contract = kernel.effective_property_contract(descriptor_key)
        populated = holon.get("properties", {})
        described = {ref.rsplit(".", 1)[0]: ref for ref in contract}

        # DS-PROP-001 -- required-property presence, gated by EnforceMinimum.
        for property_ref in contract:
            name = property_ref.rsplit(".", 1)[0]
            if kernel.effective_property_value(property_ref, "IsValueRequired") is not True:
                continue
            if not kernel.enforce_minimum(holon, property_ref):
                continue
            if name in populated:
                continue
            if kernel.effective_property_value(property_ref, "DefaultValue") is not None:
                continue  # populate_defaults() fills this before validation
            findings.append(Finding("DS-PROP-001", package, key, name))

        # DS-PROP-003 -- no undescribed populated properties.
        if not kernel.allows_additional_properties(descriptor_key):
            for name in populated:
                if name not in described:
                    findings.append(Finding("DS-PROP-003", package, key, name))

        # Native BaseValue-versus-ValueType kind compatibility.
        for name, value in populated.items():
            property_ref = described.get(name)
            if property_ref is None:
                continue  # already reported by DS-PROP-003
            family = kernel.value_family(property_ref)
            if family is None:
                findings.append(
                    Finding("BaseValueKind", package, key, f"{name}: no ValueType selected")
                )
                continue
            if family.startswith("Unsupported"):
                findings.append(Finding("BaseValueKind", package, key, f"{name}: {family}"))
                continue
            if family in ("AnyBaseValue", "ValueArray"):
                continue
            kind = native_kind(value)
            compatible = (
                family == kind
                or (family == "Enum" and kind == "String")
                or (family == "Bytes" and kind == "String")
            )
            if not compatible:
                findings.append(
                    Finding(
                        "BaseValueKind",
                        package,
                        key,
                        f"{name}: descriptor={family} value={kind}",
                    )
                )

    return findings


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--imports",
        type=pathlib.Path,
        default=pathlib.Path(__file__).resolve().parent.parent / "generated" / "json-imports",
        help="root of the generated loader JSON (default: generated/json-imports)",
    )
    parser.add_argument(
        "-v", "--verbose", action="store_true", help="list every finding instead of the first few"
    )
    args = parser.parse_args()

    holons = load_corpus(args.imports)
    kernel = Kernel(holons)
    findings = assess(kernel)

    descriptor_holons = sum(1 for h in holons.values() if str(h.get("type", "")).startswith("Meta"))
    bindings = sum(len(kernel.targets(key, "ValidationBindings")) for key in holons)
    constraints = sum(len(kernel.targets(key, "Constraints")) for key in holons)

    print(f"corpus:      {len(holons)} holons ({descriptor_holons} descriptor holons)")
    print(f"universal:   {sorted(m.rsplit('.', 1)[0] for m in kernel.universal_members)}")
    print(f"bindings:    {bindings} ValidationBindings occurrences")
    print(f"constraints: {constraints} Constraints occurrences")

    if not findings:
        print("\nno predicted findings")
        return 0

    by_rule = collections.Counter(f.rule for f in findings)
    print(f"\n{len(findings)} predicted findings:")
    for rule, count in by_rule.most_common():
        print(f"\n  {rule}: {count}")
        matching = [f for f in findings if f.rule == rule]
        by_detail = collections.Counter(f.detail for f in matching)
        for detail, detail_count in by_detail.most_common():
            print(f"    {detail} x{detail_count}")
            examples = [f for f in matching if f.detail == detail]
            for finding in examples if args.verbose else examples[:3]:
                print(f"        {finding.package} :: {finding.holon}")
            if not args.verbose and len(examples) > 3:
                print(f"        ... and {len(examples) - 3} more (-v to list)")
    return 1


if __name__ == "__main__":
    sys.exit(main())
