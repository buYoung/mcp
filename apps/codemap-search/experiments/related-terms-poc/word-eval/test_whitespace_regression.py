"""Regression for raw padding bypass; splitting/escape semantics are intentionally fixed."""
import itertools
import re
import unittest
from collect_full import scan, value_decision


class RawWhitespaceRegression(unittest.TestCase):
    def test_python_raw_padding_excluded(self):
        source = b'@named(" padded ", other="plain", empty="", spaces=" ")\ndef real(): pass\n'
        counts, _, _, _, _ = scan(source, 'python', False)
        self.assertEqual(dict(counts['tag_value']), {'plain': 1})

    def test_go_raw_padding_excluded(self):
        source = b'package example\ntype Item struct { A string `json:" padded " xml:"plain"` }\n'
        self.assertEqual(dict(scan(source, 'go', False)[0]['tag_value']), {'plain': 1})

    def test_tabs_newlines_and_internal_whitespace_excluded(self):
        source = b'@named("\tpad", suffix="pad\t", middle="two words", plain="a,b|c=d")\ndef real(): pass\n'
        self.assertEqual(dict(scan(source, 'python', False)[0]['tag_value']), {'a': 1, 'b': 1, 'c': 1, 'd': 1})

    def test_decision_difference_is_only_former_padding_bypass(self):
        for size in range(5):
            for chars in itertools.product('a ,\t\n\u00a0', repeat=size):
                value = ''.join(chars)
                old = ('empty' if not value.strip() else
                       'has_whitespace' if re.search(r'\s', value.strip()) else None)
                new = value_decision(value, False)
                if old != new:
                    self.assertIsNone(old)
                    self.assertEqual(new, 'has_whitespace')
                    self.assertNotEqual(value, value.strip())
                if new is None:
                    self.assertFalse(re.search(r'\s', value))
                self.assertEqual(value_decision(value, True), 'test_file')

    def test_literal_escape_not_decoded_and_test_values_excluded(self):
        source = b'@named("literal\\t", other="a,b")\ndef real(): pass\n'
        self.assertEqual(dict(scan(source, 'python', False)[0]['tag_value']), {'literal\\t': 1, 'a': 1, 'b': 1})
        self.assertFalse(scan(source, 'python', True)[0]['tag_value'])

if __name__ == '__main__':
    unittest.main()
