"""Rebuild realistic Go, Rust, Java and HTML scenario inputs, without dependencies.

Hand-written application code precedes BEGIN GENERATED SCENARIOS in each source.
Only ordinary data layout is compacted to reach 10,000 lines; no filler is added.
The independent manifests identify the exact 900 sensitive value spans.
"""
import html
import json
import re
import sys
from dataclasses import dataclass

sys.dont_write_bytecode = True

from fixture_data import FIELDS, REGIONS, TENANTS, OUT, grouped, international, values


@dataclass
class Sensitive:
    entity: str
    value: str


@dataclass
class Record:
    kind: str
    fields: dict


def snake(name):
    return re.sub(r'(?<!^)([A-Z])', r'_\1', name).lower()


def quote(value):
    return json.dumps(value, ensure_ascii=False)


def scenario_data(index, tenant):
    slug, display, sector, contact, channel = tenant
    def identifiers(entities):
        return {FIELDS[entity]: Sensitive(entity, values[entity][index % len(values[entity])]) for entity in entities}
    merchants = []
    for region_index, (code, name, language, city, district, street, zone, currency) in enumerate(REGIONS):
        merchants.append(Record('MerchantInput', {
            'externalReference': f'{slug}-{code.lower()}-branch', 'displayName': f'{display} {city}',
            'country': code, 'locale': language+'-'+code, 'currency': currency,
            'contactName': contact, 'contactRole': 'Regional operations manager',
            'identifiers': Record('Documents_'+code, identifiers(grouped[code])),
            'address': Record('Address', dict(city=city, district=district, streetName=street,
                buildingNumber=str(12+index), unitName='Main office', timeZone=zone)),
            'business': Record('Business', dict(sector=sector, legalForm='company' if index%2 else 'partnership',
                foundedYear=2005+index, employeeCount=12+region_index, annualTurnoverCents=12500000+index*100000,
                averageOrderCents=2400+region_index*100, hasPhysicalStore=True, hasOnlineStore=index%3!=0)),
            'preferences': Record('Preferences', dict(language=language, invoiceDelivery='portal' if index%2 else 'email',
                notificationMode='digest' if region_index%2 else 'immediate', hasMarketingConsent=False, hasServiceConsent=True)),
            'limits': Record('Limits', dict(singlePaymentCents=50000, dailyPaymentCents=250000,
                refundWindowDays=30, settlementDelayDays=2)),
            'capabilities': Record('Capabilities', dict(canAcceptPayments=True, canIssueRefunds=True,
                canRequestPayouts=True, shouldRequireSecondReviewer=sector=='healthcare')),
            'tags': [sector, channel, 'priority' if index%2 else 'standard'],
            'statementDescriptor': slug[:20], 'supportQueue': f'{language}-merchant-support', 'onboardingChannel': channel,
        }))
    return Record('Scenario', {
        'tenant': Record('Tenant', dict(slug=slug, displayName=display, defaultCurrency='USD', defaultLocale='en',
            enabledCountries=[r[0] for r in REGIONS], plan='enterprise' if index%2 else 'standard',
            monthlyDocumentLimit=5000, isExportEnabled=True)),
        'verification': Record('Verification', identifiers(international)),
        'paymentAmountCents': 18000+index*100, 'partialRefundCents': 1500+index*100, 'payoutAmountCents': 5000,
        'expectedMerchantCount': 18, 'expectedDocumentCount': 81, 'merchants': merchants,
    })


SCENARIOS = [scenario_data(index, tenant) for index, tenant in enumerate(TENANTS)]
COMPACT_KINDS = {'Address', 'Business', 'Preferences', 'Limits', 'Capabilities'}
FILES = {
    'go': ('merchant_service.go', 'merchant_service.go.pii.json'),
    'rust': ('merchant_service.rs', 'merchant_service.rs.pii.json'),
    'java': ('MerchantService.java', 'MerchantService.java.pii.json'),
    'html': ('merchant_dashboard.html', 'merchant_dashboard.html.pii.json'),
}


class Writer:
    def __init__(self, language, compact):
        self.language = language
        self.compact = compact
        name = FILES[language][0]
        text = (OUT / name).read_text()
        marker_line = next(line for line in text.splitlines() if 'BEGIN GENERATED SCENARIOS' in line)
        self.lines = text.split(marker_line, 1)[0].splitlines() + [marker_line]
        self.spans = []
        self.savings = []

    def emit(self, text=''):
        self.lines.append(text)

    def sensitive(self, prefix, value, suffix, field):
        self.spans.append(dict(entity=value.entity, field=field, line=len(self.lines)+1,
                               column_bytes=len(prefix.encode()), value=value.value))
        self.emit(prefix+value.value+suffix)

    def ordinary_block(self, lines):
        index = len(self.savings)
        self.savings.append(len(lines)//2)
        width = 2 if index in self.compact else 1
        for offset in range(0, len(lines), width):
            self.emit(lines[offset] + ''.join(' '+line.lstrip() for line in lines[offset+1:offset+width]))

    def literal(self, value):
        if isinstance(value, list):
            content = ', '.join(quote(item) for item in value)
            return {'go':'[]string{'+content+'}', 'rust':'vec!['+content+']', 'java':'List.of('+content+')'}[self.language]
        return quote(value)

    def field(self, name):
        if self.language == 'go':
            return name[0].upper()+name[1:]
        return snake(name)

    def record(self, record, indent, prefix, suffix):
        is_documents = record.kind.startswith('Documents_')
        kind = record.kind
        if is_documents:
            kind = 'map[string]string' if self.language == 'go' else 'CountryDocuments::'+kind.split('_')[1]
        self.emit(' '*indent+prefix+kind+' {')
        body = ' '*(indent+4)
        ordinary = []
        for name, value in record.fields.items():
            field = quote(name) if is_documents and self.language == 'go' else self.field(name)
            lead = field+': '
            if isinstance(value, Sensitive):
                self.sensitive(body+lead+'"', value, '",', self.field(name) if self.language=='rust' else name)
            elif isinstance(value, Record):
                self.record(value, indent+4, lead, ',')
            elif isinstance(value, list) and value and isinstance(value[0], Record):
                self.emit(body+lead+('[]MerchantInput{' if self.language=='go' else 'vec!['))
                for item in value:
                    self.record(item, indent+8, '', ',')
                self.emit(body+('},' if self.language=='go' else '],'))
            else:
                line = body+lead+self.literal(value)+','
                if record.kind in COMPACT_KINDS: ordinary.append(line)
                else: self.emit(line)
        if ordinary: self.ordinary_block(ordinary)
        self.emit(' '*indent+'}'+suffix)


def rust_documents(writer):
    writer.emit('#[derive(Clone, Debug)]')
    writer.emit('enum CountryDocuments {')
    for code, entities in grouped.items():
        writer.emit('    '+code+' {')
        for entity in entities: writer.emit('        '+snake(FIELDS[entity])+": &'static str,")
        writer.emit('    },')
    writer.emit('}')
    writer.emit('impl CountryDocuments {')
    writer.emit("    fn country(&self) -> &'static str {")
    writer.emit('        match self {')
    for code in grouped: writer.emit(f'            Self::{code} {{ .. }} => "{code}",')
    writer.emit('        }')
    writer.emit('    }')
    writer.emit("    fn fields(&self) -> Vec<(&'static str, &'static str)> {")
    writer.emit('        match self {')
    for code, entities in grouped.items():
        fields = [snake(FIELDS[e]) for e in entities]
        writer.emit('            Self::'+code+' { '+', '.join(fields)+' } => vec![')
        for field in fields: writer.emit(f'                ("{field}", *{field}),')
        writer.emit('            ],')
    writer.emit('        }')
    writer.emit('    }')
    writer.emit('}')


def emit_native(writer):
    if writer.language == 'rust': rust_documents(writer)
    factories = []
    for tenant, scenario in zip(TENANTS, SCENARIOS):
        slug = tenant[0]
        factory = ('Build'+''.join(part.title() for part in slug.split('-'))+'Scenario'
                   if writer.language=='go' else 'build_'+slug.replace('-', '_')+'_scenario')
        factories.append(factory)
        writer.emit()
        writer.emit(f'func {factory}() Scenario {{' if writer.language=='go' else f'fn {factory}() -> Scenario {{')
        writer.record(scenario, 4, 'return ' if writer.language=='go' else '', '')
        writer.emit('}')
    writer.emit()
    writer.emit('func BuildScenarios() []Scenario {' if writer.language=='go' else 'fn build_scenarios() -> Vec<Scenario> {')
    writer.emit('    return []Scenario{' if writer.language=='go' else '    vec![')
    for factory in factories: writer.emit('        '+factory+'(),')
    writer.emit('    }' if writer.language=='go' else '    ]')
    writer.emit('}')


def emit_java(writer):
    fields = sorted({FIELDS[entity] for entities in grouped.values() for entity in entities})
    writer.emit('    static final class DocumentSet {')
    for field in fields: writer.emit(f'        String {field};')
    writer.emit('        Map<String, String> fields() {')
    writer.emit('            Map<String, String> fields = new LinkedHashMap<>();')
    for field in fields: writer.emit(f'            if ({field} != null) fields.put("{field}", {field});')
    writer.emit('            return fields;')
    writer.emit('        }')
    writer.emit('    }')
    factories = []
    for tenant, scenario in zip(TENANTS, SCENARIOS):
        factory = 'build'+''.join(part.title() for part in tenant[0].split('-'))+'Scenario'
        factories.append(factory)
        writer.emit()
        writer.emit(f'    static Scenario {factory}() {{')
        writer.emit('        Tenant tenant = new Tenant(')
        tenant_values = list(scenario.fields['tenant'].fields.values())
        for i, value in enumerate(tenant_values):
            writer.emit('            '+writer.literal(value)+(',' if i<len(tenant_values)-1 else ''))
        writer.emit('        );')
        writer.emit('        Verification verification = new Verification();')
        for field, value in scenario.fields['verification'].fields.items():
            writer.sensitive(f'        verification.{field} = "', value, '";', field)
        writer.emit('        List<MerchantInput> merchants = new ArrayList<>();')
        for merchant in scenario.fields['merchants']:
            writer.emit('        {')
            writer.emit('            MerchantInput input = new MerchantInput();')
            for field, value in merchant.fields.items():
                if isinstance(value, Record):
                    if value.kind.startswith('Documents_'):
                        writer.emit('            input.identifiers = new DocumentSet();')
                        for name, sensitive in value.fields.items():
                            writer.sensitive(f'            input.identifiers.{name} = "', sensitive, '";', name)
                    else:
                        writer.emit(f'            input.{field} = new {value.kind}(')
                        entries = list(value.fields.values())
                        writer.ordinary_block(['                '+writer.literal(item)+(',' if i<len(entries)-1 else '') for i, item in enumerate(entries)])
                        writer.emit('            );')
                else:
                    writer.emit(f'            input.{field} = '+writer.literal(value)+';')
            writer.emit('            merchants.add(input);')
            writer.emit('        }')
        writer.emit('        return new Scenario(tenant, verification,')
        writer.emit('            '+', '.join(str(scenario.fields[field]) for field in ('paymentAmountCents','partialRefundCents','payoutAmountCents','expectedMerchantCount','expectedDocumentCount'))+', List.copyOf(merchants));')
        writer.emit('    }')
    writer.emit('    static List<Scenario> buildScenarios() {')
    writer.emit('        return List.of(')
    for index, factory in enumerate(factories): writer.emit('            '+factory+'()'+(',' if index<len(factories)-1 else ''))
    writer.emit('        );')
    writer.emit('    }')
    writer.emit('}')


def html_input(field, value, indent='              ', is_sensitive=False):
    label = re.sub(r'([A-Z])', r' \1', field).capitalize()
    if isinstance(value, bool):
        control = f'<input type="checkbox" name="{field}"'+(' checked' if value else '')+'>'
        return f'{indent}<label>{control}{label}</label>'
    input_type = ' type="number"' if isinstance(value, int) else ''
    classes = ' class="pii-value"' if is_sensitive else ''
    escaped = html.escape(str(value), quote=True)
    return f'{indent}<label>{label}<input{input_type} name="{field}" value="{escaped}"{classes}></label>'


def emit_html(writer):
    def sensitive(field, value, indent):
        line = html_input(field, value.value, indent, True)
        prefix, suffix = line.split(html.escape(value.value, quote=True), 1)
        encoded = Sensitive(value.entity, html.escape(value.value, quote=True))
        writer.sensitive(prefix, encoded, suffix, field)
    for tenant, scenario in zip(TENANTS, SCENARIOS):
        slug, display, *_ = tenant
        writer.emit(f'    <section class="tenant" data-tenant="{slug}" aria-labelledby="{slug}-heading">')
        writer.emit(f'      <div class="tenant-heading"><h2 id="{slug}-heading">{display}</h2><span>International merchant portfolio</span></div>')
        writer.emit('      <details><summary>Tenant verification</summary>')
        writer.emit(f'        <form class="verification" data-tenant="{slug}">')
        for field, value in scenario.fields['verification'].fields.items(): sensitive(field, value, '          ')
        writer.emit('          <button type="submit">Save verification</button>')
        writer.emit('        </form>')
        writer.emit('      </details>')
        writer.emit('      <div class="merchant-grid">')
        for merchant in scenario.fields['merchants']:
            reference = merchant.fields['externalReference']
            writer.emit(f'        <article class="merchant-card" data-reference="{reference}" data-payment-cents="{scenario.fields["paymentAmountCents"]}" data-refund-cents="{scenario.fields["partialRefundCents"]}" data-payout-cents="5000">')
            writer.emit(f'          <div class="card-heading"><h3>{merchant.fields["displayName"]}</h3><span class="status" data-state="unregistered">unregistered</span></div>')
            writer.emit('          <form class="merchant-form"><details><summary>Merchant profile and documents</summary>')
            for field, value in merchant.fields.items():
                if isinstance(value, Record):
                    is_documents = value.kind.startswith('Documents_')
                    writer.emit('              <fieldset'+(' data-kind="identifiers"' if is_documents else '')+'><legend>'+('Identity documents' if is_documents else value.kind)+'</legend>')
                    if is_documents:
                        for name, item in value.fields.items(): sensitive(name, item, '                ')
                    else:
                        writer.ordinary_block([html_input(name, item, '                ') for name, item in value.fields.items()])
                    writer.emit('              </fieldset>')
                else:
                    writer.emit(html_input(field, ', '.join(value) if isinstance(value, list) else value))
            writer.emit('          </details><button type="submit">Register merchant</button></form>')
            writer.emit('          <output class="balance">Available balance: 0 cents</output>')
            writer.emit('          <div class="actions">')
            for actions in [('submit','approve'),('authorize','capture'),('refund','payout')]:
                writer.emit('            '+' '.join(f'<button type="button" class="secondary" data-action="{action}">{action.capitalize()}</button>' for action in actions))
            writer.emit('          </div></article>')
        writer.emit('      </div>')
        writer.emit('    </section>')
    writer.emit('    <section aria-labelledby="audit-heading"><h2 id="audit-heading">Recent activity</h2><ol id="audit-log"></ol></section>')
    writer.emit('  </main>')
    writer.emit('  <footer>Merchant operations · Synthetic local demonstration</footer>')
    writer.emit('</body>')
    writer.emit('</html>')


def render(language, compact):
    writer = Writer(language, compact)
    if language in ('go', 'rust'): emit_native(writer)
    elif language == 'java': emit_java(writer)
    else: emit_html(writer)
    return writer


def rebuild(language, is_check):
    expanded = render(language, set())
    remaining = len(expanded.lines) - 10000
    assert remaining >= 0, (language, len(expanded.lines))
    reachable = {0: ()}
    for index, saving in enumerate(expanded.savings):
        for total, selected in list(reachable.items())[::-1]:
            if total+saving <= remaining and total+saving not in reachable:
                reachable[total+saving] = selected+(index,)
        if remaining in reachable: break
    assert remaining in reachable, (language, remaining, max(reachable))
    writer = render(language, set(reachable[remaining]))
    assert len(writer.lines) == 10000
    assert len(writer.spans) == 900
    assert len({span['entity'] for span in writer.spans}) == 90
    assert len({span['line'] for span in writer.spans}) == 900
    source = '\n'.join(writer.lines)+'\n'
    for span in writer.spans:
        line = writer.lines[span['line']-1].encode()
        assert line[span['column_bytes']:span['column_bytes']+len(span['value'].encode())].decode() == span['value']
    manifest = dict(line_count=10000, pii_occurrences=900, entities=list(FIELDS), spans=writer.spans)
    source_name, manifest_name = FILES[language]
    for name, content in [(source_name, source), (manifest_name, json.dumps(manifest, ensure_ascii=False, indent=2)+'\n')]:
        if is_check: assert (OUT/name).read_text() == content, f'{name} differs from regeneration'
        else: (OUT/name).write_text(content)
    print(json.dumps(dict(language=language, lines=10000, bytes=len(source.encode()), pii_occurrences=900,
        entities=90, expanded_lines=len(expanded.lines), compacted_objects=len(reachable[remaining]))))


if __name__ == '__main__':
    for language in FILES: rebuild(language, '--check' in sys.argv)
