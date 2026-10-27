"""Verify measurement grouping and reject captures that would misstate a baseline."""

import importlib.util
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("measure", Path(__file__).with_name("measure-commit-performance.py"))
measure = importlib.util.module_from_spec(spec)
spec.loader.exec_module(measure)


def complete_run():
    lines = []
    for sample in range(6):
        condition = "first" if sample == 0 else "repeated"
        lines.extend([
            f"[commit-baseline] sample_start={sample} condition={condition}",
            "[commit-assessment] phase=ReadinessResolution elapsed_us=90 backend_calls=123 link_queries=123 exact_node_reads=57",
            "[commit-assessment] phase=PackageDiscovery elapsed_us=100 backend_calls=9",
            "[commit-assessment] phase=PreparedProducts elapsed_us=20 backend_calls=0",
            "[commit-adapter] total_us=200",
            "[commit-transport] conductor_call_us=220 success=true",
            "[commit-construction] elapsed_us=150 backend_calls=201 link_queries=201 exact_node_reads=80",
            "[commit-preparation] distinct_content=10 contract_consumers=1",
            "[commit-preparation-type] type=MetaPropertyType.MetaTypeDescriptor distinct_content=3 identity_role_visits=4 backend_calls=2 exact_node_reads=1",
            "[commit-preparation-operation] operation=MembershipPrefetch role=Commitment elapsed_us=80",
            "[commit-preparation-operation] operation=EffectiveContract role=PreparedDefinition elapsed_us=15",
            "[commit-preparation-operation] operation=ContentRetention role=SavedContent exact_node_reads=3",
            "[PERF-688] guest_commit: validation_ms=1",
            f"[commit-baseline] sample_end={sample} condition={condition} command_us={250 + sample}",
        ])
    return "\n".join(lines)


class CaptureTests(unittest.TestCase):
    def test_link_profile_wires_separate_output_and_records_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "capture"
            argv = ["measure", "--runs", "1", "--link-profile", "--output", str(destination)]

            def capture(command, *, stdout, stderr, env):
                self.assertIn("--exact", command)
                self.assertIn("measures_first_and_repeated_visualizer_usage_commits", command)
                self.assertNotIn("MAP_COMMIT_PERF_INDEX_MODE", env)
                self.assertEqual(env["MAP_COMMIT_PERF_LINK_OUTPUT"], str((destination / "run-01-links.json").resolve()))
                stdout.write(complete_run())
                return SimpleNamespace(returncode=0)

            with patch.object(measure.sys, "argv", argv), \
                 patch.dict(measure.os.environ, {"MAP_COMMIT_PERF_LINK_OUTPUT": "stale.json"}, clear=True), \
                 patch.object(measure, "output", return_value=""), \
                 patch.object(measure.platform, "platform", return_value="test-platform"), \
                 patch.object(measure.platform, "machine", return_value="test-machine"), \
                 patch.object(measure.subprocess, "run", side_effect=capture), \
                 patch.object(measure, "locked_version", side_effect=lambda path, package: f"{path}:{package}"), \
                 patch("builtins.print"):
                self.assertEqual(measure.main(), 0)
            metadata = json.loads((destination / "metadata.json").read_text())
            self.assertTrue(metadata["link_profile"])
            self.assertEqual(metadata["anchor_evidence_policy"], "per-new-version")
            self.assertEqual(metadata["package_substrate"], "DescriptorPackage")
            self.assertEqual(metadata["runtime_versions"], {"holochain": "Cargo.lock:holochain",
                                                            "hdk": "happ/Cargo.lock:hdk",
                                                            "hdi": "happ/Cargo.lock:hdi"})
            self.assertNotIn("index_mode", metadata)

    def test_new_creation_modes_and_readiness_controls_are_selected_in_capture(self):
        for mode in ["visualizer-slot", "descriptor"]:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory:
                destination = Path(directory) / "capture"
                argv = ["measure", "--runs", "1", "--mode", mode, "--output", str(destination)]
                if mode == "visualizer-slot":
                    argv.append("--readiness-cases")

                def capture(command, *, stdout, stderr, env):
                    self.assertEqual(env["MAP_COMMIT_PERF_MODE"], mode)
                    self.assertNotIn("MAP_COMMIT_PERF_LINK_OUTPUT", env)
                    if mode == "visualizer-slot":
                        self.assertEqual(env["MAP_COMMIT_READINESS_CASES_OUTPUT"],
                                         str((destination / "run-01-cases.json").resolve()))
                    else:
                        self.assertNotIn("MAP_COMMIT_READINESS_CASES_OUTPUT", env)
                    stdout.write(complete_run())
                    return SimpleNamespace(returncode=0)

                with patch.object(measure.sys, "argv", argv), \
                     patch.dict(measure.os.environ, {"MAP_COMMIT_READINESS_CASES_OUTPUT": "stale.json"}, clear=True), \
                     patch.object(measure, "output", return_value=""), \
                     patch.object(measure.platform, "platform", return_value="test-platform"), \
                     patch.object(measure.platform, "machine", return_value="test-machine"), \
                     patch.object(measure.subprocess, "run", side_effect=capture), \
                     patch.object(measure, "locked_version", return_value="test-version"), \
                     patch("builtins.print"):
                    self.assertEqual(measure.main(), 0)
                metadata = json.loads((destination / "metadata.json").read_text())
                self.assertEqual(metadata["mode"], mode)
                self.assertEqual(metadata["readiness_cases"], mode == "visualizer-slot")

    def test_archived_capture_metadata_selects_mode_samples_and_evidence_policy(self):
        marker = "[commit-preparation-operation] operation=saved_lineage_roots_by_key role=anchor-evidence invocations=1 backend_calls=51 link_queries=51\n"
        for policy in ["per-new-version", "retired", None]:
            with self.subTest(policy=policy), tempfile.TemporaryDirectory() as directory:
                destination = Path(directory)
                metadata = {"mode": "new-version", "samples_per_run": 2,
                            "variant": "prefetch", "assessment": "standard"}
                if policy is not None:
                    metadata["anchor_evidence_policy"] = policy
                (destination / "metadata.json").write_text(json.dumps(metadata))
                content = complete_run().split("[commit-baseline] sample_start=2")[0]
                if policy != "retired":
                    content = content.replace("[PERF-688]", marker + "[PERF-688]")
                (destination / "run-01.log").write_text(content)
                with patch.object(measure.sys, "argv", ["measure", "--summarize-only", "--output", str(destination)]), \
                     patch("builtins.print"):
                    self.assertEqual(measure.main(), 0)
                result = json.loads((destination / "measurements.json").read_text())
                self.assertEqual(result["mode"], "new-version")
                self.assertEqual(result["anchor_evidence_policy"], policy or "per-new-version")
                self.assertEqual(len(result["samples"]), 2)

    def test_descriptor_mode_rejects_instance_controls_and_multiple_subjects(self):
        for extra in [["--readiness-cases"], ["--subjects", "2"], ["--link-profile"]]:
            with self.subTest(extra=extra), tempfile.TemporaryDirectory() as directory:
                destination = Path(directory) / "capture"
                with patch.object(measure.sys, "argv", ["measure", "--mode", "descriptor", "--output", str(destination), *extra]), \
                     patch.object(measure.sys, "stderr"), \
                     self.assertRaises(SystemExit) as error:
                    measure.main()
                self.assertEqual(error.exception.code, 2)
                self.assertFalse(destination.exists())

    def test_retired_index_environment_is_rejected_before_creating_output(self):
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "capture"
            with patch.object(measure.sys, "argv", ["measure", "--output", str(destination)]), \
                 patch.dict(measure.os.environ, {"MAP_COMMIT_PERF_INDEX_MODE": "base-hash"}, clear=True), \
                 patch.object(measure.sys, "stderr"), \
                 self.assertRaises(SystemExit) as error:
                measure.main()
            self.assertEqual(error.exception.code, 2)
            self.assertFalse(destination.exists())

    def summarize(self, contents):
        with tempfile.TemporaryDirectory() as directory:
            logs = []
            for index, content in enumerate(contents):
                log = Path(directory) / f"run-{index}.log"
                log.write_text(content)
                logs.append(log)
            return measure.summarize(logs)

    def test_conditions_phases_and_operations_remain_distinct(self):
        result = self.summarize([complete_run(), complete_run()])
        self.assertEqual(result["summary"]["first"]["commit-baseline.command_us"]["n"], 2)
        self.assertEqual(result["summary"]["repeated"]["commit-baseline.command_us"]["n"], 10)
        metrics = result["samples"][0]["metrics"]
        self.assertEqual(metrics["commit-assessment.ReadinessResolution.link_queries"], 123)
        self.assertEqual(metrics["commit-assessment.ReadinessResolution.exact_node_reads"], 57)
        self.assertEqual(metrics["commit-construction.backend_calls"], 201)
        self.assertEqual(metrics["commit-construction.exact_node_reads"], 80)
        self.assertEqual(metrics["commit-assessment.PackageDiscovery.elapsed_us"], 100)
        self.assertEqual(metrics["commit-assessment.PreparedProducts.elapsed_us"], 20)
        self.assertEqual(metrics["commit-preparation-operation.MembershipPrefetch.Commitment.elapsed_us"], 80)
        self.assertEqual(metrics["commit-preparation-type.MetaPropertyType.MetaTypeDescriptor.distinct_content"], 3)
        self.assertEqual(metrics["commit-preparation-type.MetaPropertyType.MetaTypeDescriptor.exact_node_reads"], 1)

    def test_exclusive_components_remain_distinct_and_reconcile(self):
        costs = ("[commit-read-cost] phase=Construction component=LinkPrefix elapsed_us=90\n"
                 "[commit-read-cost] phase=Construction component=OtherConstruction elapsed_us=59\n"
                 "[commit-cost-accounting] construction_us=150 accounted_us=149 timer_gap_us=1\n")
        contents = complete_run().replace("[PERF-688]", costs + "[PERF-688]")
        result = self.summarize([contents])
        self.assertEqual(result["samples"][0]["metrics"]["commit-read-cost.Construction.LinkPrefix.elapsed_us"], 90)
        with self.assertRaisesRegex(ValueError, "Inconsistent exclusive"):
            self.summarize([contents.replace("component=LinkPrefix elapsed_us=90", "component=LinkPrefix elapsed_us=190")])

    def test_missing_operation_measurement_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "Missing measurements"):
            self.summarize([complete_run().replace("[commit-preparation-operation] operation=MembershipPrefetch role=Commitment elapsed_us=80", "")])

    def test_assessment_label_matches_guest_path(self):
        marker = "[commit-preparation-operation] operation=RelationshipDerivedInputs role=InstanceExperiment elapsed_us=25\n"
        content = complete_run().replace("[PERF-688]", marker + "[PERF-688]")
        with self.assertRaisesRegex(ValueError, "Guest assessment path"):
            self.summarize([content])
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "run.log"
            log.write_text(content)
            result = measure.summarize([log], assessment="instance-experiment")
            self.assertEqual(result["assessment"], "instance-experiment")
            log.write_text(complete_run())
            with self.assertRaisesRegex(ValueError, "Guest assessment path"):
                measure.summarize([log], assessment="instance-experiment")

    def test_validator_package_labels_require_the_matching_guest_builder(self):
        package = "[commit-preparation-operation] operation=ValidatorPackageConstruction role=ValidatorPackage elapsed_us=50\n"
        instance = "[commit-preparation-operation] operation=RelationshipDerivedInputs role=InstanceExperiment elapsed_us=25\n"
        content = complete_run().replace("role=Commitment", "role=ValidatorRoot").replace("[PERF-688]", package + "[PERF-688]")
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "run.log"
            log.write_text(content)
            self.assertEqual(measure.summarize([log], assessment="validator-package")["assessment"], "validator-package")
            log.write_text(content.replace("[PERF-688]", instance + "[PERF-688]"))
            self.assertEqual(measure.summarize([log], assessment="combined")["assessment"], "combined")
            log.write_text(content.replace("role=ValidatorRoot", "role=Commitment"))
            with self.assertRaisesRegex(ValueError, "Missing measurements"):
                measure.summarize([log], assessment="validator-package")

    def test_warm_probe_requires_three_measured_passes(self):
        markers = ("[commit-preparation-operation] operation=RelationshipDerivedInputs role=InstanceExperiment elapsed_us=25\n"
                   "[commit-preparation-operation] operation=WarmValidatorAssessment role=SameTransactionProbe invocations=3 elapsed_us=12 backend_calls=0 link_queries=0 exact_node_reads=0\n")
        content = complete_run().replace("role=Commitment", "role=ValidatorRoot").replace("[PERF-688]", markers + "[PERF-688]")
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "run.log"
            log.write_text(content)
            result = measure.summarize([log], assessment="warm-probe")
            self.assertEqual(result["assessment"], "warm-probe")
            with self.assertRaisesRegex(ValueError, "Guest warm probe"):
                measure.summarize([log], assessment="combined")
            log.write_text(content.replace("invocations=3", "invocations=2"))
            with self.assertRaisesRegex(ValueError, "Expected three warm"):
                measure.summarize([log], assessment="warm-probe")

    def test_new_version_mode_requires_one_anchor_evidence_load(self):
        marker = "[commit-preparation-operation] operation=saved_lineage_roots_by_key role=anchor-evidence elapsed_us=1700 invocations=1 backend_calls=51 link_queries=51\n"
        content = complete_run().replace("[PERF-688]", marker + "[PERF-688]")
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "run.log"
            log.write_text(content)
            result = measure.summarize([log], mode="new-version")
            self.assertEqual(result["mode"], "new-version")
            self.assertEqual(result["samples"][0]["metrics"]["commit-preparation-operation.saved_lineage_roots_by_key.anchor-evidence.backend_calls"], 51)
            with self.assertRaisesRegex(ValueError, "Anchor evidence does not match create"):
                measure.summarize([log])
            log.write_text(complete_run())
            with self.assertRaisesRegex(ValueError, "Anchor evidence does not match new-version"):
                measure.summarize([log], mode="new-version")

    def test_retired_policy_requires_zero_anchor_evidence_in_every_mode(self):
        marker = "[commit-preparation-operation] operation=saved_lineage_roots_by_key role=anchor-evidence invocations=1\n"
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "run.log"
            for mode in ["create", "visualizer-slot", "descriptor", "new-version"]:
                with self.subTest(mode=mode):
                    log.write_text(complete_run())
                    result = measure.summarize([log], mode=mode, anchor_evidence_policy="retired")
                    self.assertEqual(result["anchor_evidence_policy"], "retired")
                    log.write_text(complete_run().replace("[PERF-688]", marker + "[PERF-688]"))
                    with self.assertRaisesRegex(ValueError, "retired policy"):
                        measure.summarize([log], mode=mode, anchor_evidence_policy="retired")

    def test_locked_version_reads_the_named_package(self):
        with tempfile.TemporaryDirectory() as directory:
            lock = Path(directory) / "Cargo.lock"
            lock.write_text('[[package]]\nname = "holochain_types"\nversion = "9.9.9"\n\n'
                            '[[package]]\nname = "holochain"\nversion = "1.2.3"\n')
            self.assertEqual(measure.locked_version(lock, "holochain"), "1.2.3")
            with self.assertRaisesRegex(ValueError, "No locked version of hdk"):
                measure.locked_version(lock, "hdk")

    def test_invalid_anchor_policy_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "Unknown anchor evidence policy"):
            measure.summarize([Path("unused.log")], anchor_evidence_policy="unknown")

    def test_partial_capture_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "Unfinished sample"):
            self.summarize([complete_run().rsplit("\n", 1)[0]])

    def test_missing_whole_sample_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "Expected 6 complete samples"):
            self.summarize([complete_run().split("[commit-baseline] sample_start=5")[0]])

    def test_empty_capture_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "No captured run logs"):
            measure.summarize([])

    def test_on_demand_variant_accepts_absent_prefetch_probe(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "run.log"
            log.write_text(complete_run().replace(
                "[commit-preparation-operation] operation=MembershipPrefetch role=Commitment elapsed_us=80", ""
            ))
            result = measure.summarize([log], variant="on-demand")
            self.assertEqual(len(result["samples"]), 6)
            self.assertEqual(result["variant"], "on-demand")

    def test_on_demand_label_rejects_prefetch_capture(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "run.log"
            log.write_text(complete_run())
            with self.assertRaisesRegex(ValueError, "unexpectedly contains eager prefetch"):
                measure.summarize([log], variant="on-demand")


if __name__ == "__main__":
    unittest.main()
