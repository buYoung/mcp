import unittest
from collect_full import scan


class FullCollectionTests(unittest.TestCase):
    def test_go_values_only_no_comments_whitespace_or_tests(self):
        source = b'''package demo
// docs: json:"commentOnly"
type Example struct {
 A string `json:"real,omitempty" note:"two words" xml:"-" empty:""`
}
func run() { usedOnly.Call() }
'''
        counts, quals, _, _, skipped = scan(source, 'go', False)
        self.assertEqual(dict(counts['tag_value']), {'real':1, 'omitempty':1})
        self.assertNotIn('json', counts['tag_value'])
        self.assertNotIn('commentOnly', counts['tag_value'])
        self.assertIn('usedOnly', counts['identifier'])
        self.assertEqual(quals['identifier']['Call']['usedOnly'], 1)
        self.assertEqual(skipped['has_whitespace'], 1)
        self.assertEqual(skipped['empty'], 1)
        self.assertFalse(scan(source, 'go', True)[0]['tag_value'])

    def test_python_decorator_values_no_doc_tags(self):
        source = b'''# @route("commentOnly")
@route("/real", name="token", label="two words", empty="")
def function(arg):
    external.run(arg)
'''
        counts, _, _, _, skipped = scan(source, 'python', False)
        self.assertEqual(dict(counts['tag_value']), {'/real':1, 'token':1})
        self.assertNotIn('commentOnly', counts['identifier'])
        self.assertEqual(counts['identifier']['arg'], 2)
        self.assertEqual(skipped['has_whitespace'], 1)
        self.assertFalse(scan(source, 'python', True)[0]['tag_value'])

    def test_java_annotation_values(self):
        counts, _, _, _, _ = scan(b'@Named("actual") class Example { int value; }', 'java', False)
        self.assertEqual(dict(counts['tag_value']), {'actual':1})
        self.assertNotIn('Named', counts['tag_value'])

    def test_rust_csharp_php_attribute_values(self):
        fixtures = [
            ('rust', b'#[serde(rename = "actual")] struct Example { value: i32 }'),
            ('csharp', b'[Named("actual")] class Example { int value; }'),
            ('php', b'<?php #[Named("actual")] class Example { public $value; }'),
        ]
        for language, source in fixtures:
            with self.subTest(language=language):
                counts, _, _, _, _ = scan(source, language, False)
                self.assertEqual(dict(counts['tag_value']), {'actual':1})
                self.assertFalse(scan(source, language, True)[0]['tag_value'])

if __name__ == '__main__':
    unittest.main()
