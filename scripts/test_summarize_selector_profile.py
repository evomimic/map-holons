import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("selector_profile", Path(__file__).with_name("summarize-selector-profile.py"))
profile = importlib.util.module_from_spec(spec)
spec.loader.exec_module(profile)


def block(name, duration, fields="", request=42):
    # Match Conductora's existing pretty formatter with CLOSE events.
    return f'''  2026-10-15T20:10:00.000000Z DEBUG map_profile: close, time.busy: {duration}, time.idle: 2µs
    at fixture.rs:1
    in map_profile::{name}{' with ' + fields if fields else ''}
    in map_profile::visualizer.select with kind: Property, subject: local-1, slot: local-2
    in map_profile::ipc.dispatch with request_id: {request}

'''


class SelectorSummaryTests(unittest.TestCase):
    def test_counts_calls_and_preserves_inclusive_times_without_double_counting(self):
        text = block("storage.fetch_holon", "100µs", "source: local-3")
        text += block("conductor.zome_call", "90µs", "dance: get_holon_by_id")
        text += block("visualizer.context", "1ms")
        text += block("visualizer.select", "2ms")
        row = profile.summarize(text)["requests"][0]
        self.assertEqual(row["requestId"], 42)
        self.assertEqual(row["metadataFetches"], 1)
        self.assertEqual(row["conductorCalls"], 1)
        self.assertAlmostEqual(row["totalMs"], 2.002)
        self.assertAlmostEqual(row["phases"]["visualizer.context"]["inclusiveMs"], 1.002)

    def test_separates_requests_and_excludes_unrelated_close_events(self):
        text = block("visualizer.select", "1s", request=1)
        text += block("visualizer.select", "2ms", request=2)
        text += "2026-10-15T20:10:00Z DEBUG unrelated: close, time.busy: 3ms\n"
        rows = profile.summarize(text, [2])["requests"]
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["requestId"], 2)
        self.assertAlmostEqual(rows[0]["totalMs"], 2.002)

    def test_groups_repeated_fetches_by_source_relationship_and_phase(self):
        event = block("fetch_relationship", "1ms", "source: local-3, relationship: OffersOperator")
        event = event.replace("    in map_profile::visualizer.select", "    in map_profile::visualizer.operator_compatibility\n    in map_profile::visualizer.select")
        row = profile.summarize(event * 2)["requests"][0]
        self.assertEqual(row["metadataFetches"], 2)
        self.assertEqual(list(row["fetchesByPhaseAndIdentity"].values()), [2])
        self.assertIn("visualizer.operator_compatibility", next(iter(row["fetchesByPhaseAndIdentity"])))


if __name__ == "__main__":
    unittest.main()
