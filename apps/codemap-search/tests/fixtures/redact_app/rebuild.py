import json
import sys
sys.dont_write_bytecode = True

from fixture_data import FIELDS, REGIONS, TENANTS, OUT, grouped, international, values

source_path = OUT / "merchant_service.ts"
base = source_path.read_text().split("export const JURISDICTIONS: ReadonlyArray<JurisdictionPolicy> = [", 1)[0]

def emit_fixture(compact_blocks):
    lines=base.rstrip().splitlines()+['']
    spans=[]
    block_index=0
    def emit(line=''):
        lines.append(line)
    def scalar(key,value,indent=4):
        emit(' '*indent+key+': '+json.dumps(value,ensure_ascii=False)+',')
    def sensitive(entity,scenario_index,indent):
        value=values[entity][scenario_index % len(values[entity])]
        prefix=' '*indent+FIELDS[entity]+': "'
        spans.append({'entity':entity,'field':FIELDS[entity],'line':len(lines)+1,'column_bytes':len(prefix.encode()),'value':value})
        emit(prefix+value+'",')
    def block(name,entries,indent=8):
        nonlocal block_index
        emit(' '*indent+name+': {')
        # Ordinary compact object layout; only this formatting is varied to obtain
        # an exact-size fixture. No statements, comments or blank lines pad its size.
        is_compact=block_index in compact_blocks
        block_index+=1
        width=2 if is_compact else 1
        for offset in range(0,len(entries),width):
            part=entries[offset:offset+width]
            emit(' '*(indent+2)+' '.join(key+': '+json.dumps(value,ensure_ascii=False)+',' for key,value in part))
        emit(' '*indent+'},')

    emit('export const JURISDICTIONS: ReadonlyArray<JurisdictionPolicy> = [')
    for code,name,language,city,district,street,zone,currency in REGIONS:
        emit('  {')
        scalar('country',code)
        scalar('displayName',name)
        scalar('supportedLanguages',[language,'en'] if language!='en' else ['en'])
        scalar('supportedDocumentFields',[FIELDS[entity] for entity in grouped[code]])
        scalar('reviewQueue',f'{code.lower()}-verification')
        emit('  },')
    emit('];')
    emit()
    factory_names=[]
    for scenario_index,(slug,display,sector,contact,channel) in enumerate(TENANTS):
        factory='build'+''.join(word.title() for word in slug.split('-'))+'Scenario'
        factory_names.append(factory)
        emit(f'export function {factory}(): TenantScenario {{')
        emit('  return {')
        emit('    tenant: {')
        scalar('slug',slug,6)
        scalar('displayName',display,6)
        scalar('defaultCurrency','USD',6)
        scalar('defaultLocale','en',6)
        scalar('enabledCountries',[r[0] for r in REGIONS],6)
        scalar('plan','enterprise' if scenario_index%2 else 'standard',6)
        scalar('monthlyDocumentLimit',5000,6)
        scalar('isExportEnabled',True,6)
        emit('    },')
        emit('    verification: {')
        for entity in international:
            sensitive(entity,scenario_index,6)
        emit('    },')
        scalar('paymentAmountCents',18000+scenario_index*100,4)
        scalar('partialRefundCents',1500+scenario_index*100,4)
        scalar('payoutAmountCents',5000,4)
        scalar('expectedMerchantCount',18,4)
        scalar('expectedDocumentCount',81,4)
        emit('    merchants: [')
        for region_index,(code,name,language,city,district,street,zone,currency) in enumerate(REGIONS):
            emit('      {')
            scalar('externalReference',f'{slug}-{code.lower()}-branch',8)
            scalar('displayName',f'{display} {city}',8)
            scalar('country',code,8)
            scalar('locale',language+'-'+code,8)
            scalar('currency',currency,8)
            scalar('contactName',contact,8)
            scalar('contactRole','Regional operations manager',8)
            emit('        identifiers: {')
            for entity in grouped[code]:
                sensitive(entity,scenario_index,10)
            emit('        },')
            block('address', [('city',city),('district',district),('streetName',street),('buildingNumber',str(12+scenario_index)),('unitName','Main office'),('timeZone',zone)])
            block('business', [('sector',sector),('legalForm','company' if scenario_index%2 else 'partnership'),('foundedYear',2005+scenario_index),('employeeCount',12+region_index),('annualTurnoverCents',12500000+scenario_index*100000),('averageOrderCents',2400+region_index*100),('hasPhysicalStore',True),('hasOnlineStore',scenario_index%3!=0)])
            block('preferences', [('language',language),('invoiceDelivery','portal' if scenario_index%2 else 'email'),('notificationMode','digest' if region_index%2 else 'immediate'),('hasMarketingConsent',False),('hasServiceConsent',True)])
            block('limits', [('singlePaymentCents',50000),('dailyPaymentCents',250000),('refundWindowDays',30),('settlementDelayDays',2)])
            block('capabilities', [('canAcceptPayments',True),('canIssueRefunds',True),('canRequestPayouts',True),('shouldRequireSecondReviewer',sector=='healthcare')])
            scalar('tags',[sector,channel,'priority' if scenario_index%2 else 'standard'],8)
            scalar('statementDescriptor',slug[:20],8)
            scalar('supportQueue',f'{language}-merchant-support',8)
            scalar('onboardingChannel',channel,8)
            emit('      },')
        emit('    ],')
        emit('  };')
        emit('}')
        emit()
    emit('export const TENANT_SCENARIOS: ReadonlyArray<TenantScenario> = [')
    for factory in factory_names:
        emit('  '+factory+'(),')
    emit('];')
    assert len(spans)==900
    return lines,spans,block_index

expanded,_,block_count=emit_fixture(set())
target=10000
remaining=len(expanded)-target
assert remaining>=0,(len(expanded),target)
# Subset of object blocks chooses standard one- or two-property-per-line layouts.
# The resulting application and input values are independent of this layout choice.
savings=[3,4,2,2,2]*(block_count//5)
reachable={0:()}
for index,saving in enumerate(savings):
    for total,selected in list(reachable.items())[::-1]:
        if total+saving<=remaining and total+saving not in reachable:
            reachable[total+saving]=selected+(index,)
    if remaining in reachable:
        break
assert remaining in reachable,(remaining,max(reachable))
lines,spans,_=emit_fixture(set(reachable[remaining]))
assert len(lines)==target
source='\n'.join(lines)+'\n'
assert all(source.splitlines()[span['line']-1].encode()[span['column_bytes']:span['column_bytes']+len(span['value'].encode())].decode()==span['value'] for span in spans)
manifest={'line_count':len(lines),'pii_occurrences':len(spans),'entities':list(FIELDS),'spans':spans}
manifest_text=json.dumps(manifest,ensure_ascii=False,indent=2)+'\n'
if '--check' in sys.argv:
    assert source_path.read_text()==source,'Scenario source differs from regeneration'
    assert (OUT/'merchant_service.pii.json').read_text()==manifest_text,'Expected spans differ from regeneration'
else:
    source_path.write_text(source)
    (OUT/'merchant_service.pii.json').write_text(manifest_text)
print(json.dumps({'lines':len(lines),'bytes':len(source.encode()),'service_lines':len(base.splitlines()),'scenarios':len(TENANTS),'merchants':len(TENANTS)*len(REGIONS),'pii_occurrences':len(spans),'entities':len(FIELDS),'expanded_layout_lines':len(expanded),'compacted_objects':len(reachable[remaining])}))
