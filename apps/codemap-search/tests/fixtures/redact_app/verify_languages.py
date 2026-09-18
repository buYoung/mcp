"""Compile/run source fixtures; parse HTML and exercise its JavaScript business model.

Uses installed compilers and Node only. All build outputs are isolated in a
temporary directory; no project dependencies are installed. HTML checks do not
simulate a browser DOM or establish browser rendering/event compatibility.
"""
import argparse
import html.parser
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent


def run(command, **kwargs):
    result = subprocess.run(command, text=True, capture_output=True, timeout=180, **kwargs)
    if result.returncode:
        raise RuntimeError(f'{command[0]} exited {result.returncode}\n{result.stdout}\n{result.stderr}')
    if result.stderr.strip(): print(result.stderr.strip())
    return result.stdout


class PageParser(html.parser.HTMLParser):
    VOID = {'area','base','br','col','embed','hr','img','input','link','meta','param','source','track','wbr'}

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.stack = []
        self.ids = set()
        self.pii_inputs = 0
        self.merchant_forms = 0
        self.scripts = []
        self.forms = []
        self.current_form = None
        self.is_identifiers = False

    def handle_starttag(self, tag, attributes):
        attrs = dict(attributes)
        if 'id' in attrs:
            assert attrs['id'] not in self.ids, f'duplicate id: {attrs["id"]}'
            self.ids.add(attrs['id'])
        if tag == 'input' and attrs.get('class') == 'pii-value': self.pii_inputs += 1
        if tag == 'form' and attrs.get('class') == 'merchant-form': self.merchant_forms += 1
        if tag == 'form':
            assert self.current_form is None, 'nested form'
            self.current_form = dict(kind=attrs.get('class'), tenant=attrs.get('data-tenant'), fields={}, identifiers={})
            self.forms.append(self.current_form)
        if tag == 'fieldset': self.is_identifiers = attrs.get('data-kind') == 'identifiers'
        if tag == 'input' and self.current_form is not None and 'name' in attrs:
            fields = self.current_form['identifiers' if self.is_identifiers else 'fields']
            assert attrs['name'] not in fields, f'duplicate form field: {attrs["name"]}'
            fields[attrs['name']] = ('checked' in attrs) if attrs.get('type') == 'checkbox' else attrs.get('value', '')
        if tag not in self.VOID: self.stack.append(tag)

    def handle_endtag(self, tag):
        assert self.stack and self.stack[-1] == tag, f'unbalanced tag: {tag}, stack={self.stack}'
        self.stack.pop()
        if tag == 'form': self.current_form = None
        if tag == 'fieldset': self.is_identifiers = False

    def handle_data(self, data):
        if self.stack and self.stack[-1] == 'script': self.scripts.append(data)

    def finish(self):
        self.close()
        assert not self.stack, f'unclosed tags: {self.stack}'
        assert self.pii_inputs == 900, self.pii_inputs
        assert self.merchant_forms == 180, self.merchant_forms


def verify(language, directory):
    if language == 'go':
        env = dict(os.environ, GOCACHE=str(directory/'go-cache'), GOPROXY='off', GOSUMDB='off')
        report = run(['go', 'run', str(ROOT/'merchant_service.go')], env=env)
    elif language == 'rust':
        binary = directory/'merchant-service'
        run(['rustc', '--edition=2021', '-D', 'warnings', str(ROOT/'merchant_service.rs'), '-o', str(binary)])
        report = run([str(binary)])
    elif language == 'java':
        classes = directory/'java'
        classes.mkdir()
        run(['javac', '--release', '17', '-Xlint:all', '-Werror', '-d', str(classes), str(ROOT/'MerchantService.java')])
        report = run(['java', '-cp', str(classes), 'MerchantService'])
    else:
        source = ROOT/'merchant_dashboard.html'
        parser = PageParser()
        parser.feed(source.read_text())
        parser.finish()
        script = directory/'dashboard.js'
        script.write_text('\n'.join(parser.scripts))
        run(['node', '--check', str(script)])
        forms = directory/'forms.json'
        forms.write_text(json.dumps(parser.forms))
        report = run(['node', str(ROOT/'verify_dashboard.mjs'), str(script), str(forms)])
    result = json.loads(report)
    assert result['scenarios'] == 10 and result['merchants'] == 180 and result['documents'] == 810, result
    assert result['assertions'] > 1000, result
    print(json.dumps({'language':language, **result}), flush=True)


if __name__ == '__main__':
    arguments = argparse.ArgumentParser()
    arguments.add_argument('--only', choices=['go','rust','java','html'])
    options = arguments.parse_args()
    languages = [options.only] if options.only else ['go','rust','java','html']
    for language in languages:
        with tempfile.TemporaryDirectory(prefix=f'codemap-fixture-{language}-') as directory:
            verify(language, Path(directory))
