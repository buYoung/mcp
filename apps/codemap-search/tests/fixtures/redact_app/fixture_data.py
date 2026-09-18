"""Synthetic international examples and business inputs shared by source fixtures."""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / 'tests/fixtures/redact_app'
catalog = json.loads((ROOT/'src/redact/pii/catalog.json').read_text())
examples = [json.loads(line) for line in (ROOT/'src/redact/pii/cases.jsonl').read_text().splitlines()]

FIELDS = {
    'AU_ABN':'abn', 'AU_ACN':'acn', 'AU_MEDICARE':'medicareNumber', 'AU_TFN':'tfn',
    'CA_POSTAL_CODE':'postalCode', 'CA_SIN':'sin', 'FI_PERSONAL_IDENTITY_CODE':'personalIdentityCode',
    'DE_BSNR':'bsnr', 'DE_FUEHRERSCHEIN':'drivingLicense', 'DE_HANDELSREGISTER':'handelsregisternummer',
    'DE_HEALTH_INSURANCE':'healthInsuranceNumber', 'DE_ID_CARD':'identityCardNumber', 'DE_KFZ':'licensePlate',
    'DE_LANR':'lanr', 'DE_PASSPORT':'passportNumber', 'DE_PLZ':'postalCode', 'DE_SOCIAL_SECURITY':'socialSecurityNumber',
    'DE_TAX_ID':'taxId', 'DE_TAX_NUMBER':'taxNumber', 'DE_VAT_ID':'vatId',
    'IN_AADHAAR':'aadhaarNumber', 'IN_GSTIN':'gstin', 'IN_PAN':'pan', 'IN_PASSPORT':'passportNumber',
    'IN_VEHICLE_REGISTRATION':'vehicleRegistration', 'IN_VOTER':'voterId',
    'IT_DRIVER_LICENSE':'driverLicense', 'IT_FISCAL_CODE':'fiscalCode', 'IT_IDENTITY_CARD':'identityCardNumber',
    'IT_PASSPORT':'passportNumber', 'IT_VAT_CODE':'vatCode',
    'KR_BRN':'brn', 'KR_DRIVER_LICENSE':'driverLicense', 'KR_FRN':'frn', 'KR_PASSPORT':'passportNumber', 'KR_RRN':'rrn',
    'NG_NIN':'nin', 'NG_VEHICLE_REGISTRATION':'vehicleRegistration',
    'PH_PASSPORT':'passportNumber', 'PH_TIN':'tin', 'PH_UMID':'umid', 'PL_PESEL':'pesel',
    'SG_NRIC_FIN':'nricFin', 'SG_UEN':'uen', 'ZA_COMPANY_REGISTRATION':'companyRegistrationNumber',
    'ZA_DRIVER_LICENSE':'driverLicense', 'ZA_ID_NUMBER':'idNumber', 'ZA_INCOME_TAX_NUMBER':'incomeTaxNumber',
    'ZA_LICENSE_PLATE':'licensePlate', 'ZA_PASSPORT':'passportNumber',
    'ZA_TRAFFIC_REGISTER_NUMBER':'trafficRegisterNumber', 'ZA_VAT_NUMBER':'vatNumber',
    'ES_NIE':'nie', 'ES_NIF':'nif', 'ES_PASSPORT':'passportNumber', 'SE_ORGANISATIONSNUMMER':'organisationsnummer',
    'SE_PERSONNUMMER':'personnummer', 'TH_TNIN':'tnin', 'TR_LICENSE_PLATE':'licensePlate', 'TR_NATIONAL_ID':'nationalIdNumber',
    'UK_DRIVING_LICENCE':'drivingLicence', 'UK_NHS':'nhsNumber', 'UK_NINO':'nino', 'UK_PASSPORT':'passportNumber',
    'UK_POSTCODE':'postcode', 'UK_VEHICLE_REGISTRATION':'vehicleRegistration',
    'ABA_ROUTING_NUMBER':'routingNumber', 'MEDICAL_LICENSE':'deaNumber', 'US_BANK_NUMBER':'bankAccount',
    'US_DRIVER_LICENSE':'driverLicense', 'US_HEALTH_INSURANCE_MEMBER_ID':'memberId',
    'US_PRIOR_AUTHORIZATION_NUMBER':'priorAuthorizationNumber', 'US_CLAIM_NUMBER':'claimNumber',
    'US_PRESCRIPTION_NUMBER':'prescriptionNumber', 'US_REFERRAL_NUMBER':'referralNumber',
    'US_PROVIDER_TAX_ID':'providerTaxId', 'US_ITIN':'itinNumber', 'US_MBI':'mbiNumber', 'US_NPI':'npiNumber',
    'US_PASSPORT':'passportNumber', 'US_SSN':'ssn',
    'CREDIT_CARD':'cardNumber', 'CRYPTO':'bitcoinAddress', 'DATE_TIME':'openedAt', 'EMAIL_ADDRESS':'email',
    'IBAN_CODE':'iban', 'IP_ADDRESS':'ipAddress', 'MAC_ADDRESS':'macAddress', 'URL':'website', 'UUID':'correlationId',
}
assert set(FIELDS) == {rule['entity'] for rule in catalog}

REGIONS = [
    ('AU','Australia','en','Sydney','Surry Hills','Crown Street','Australia/Sydney','AUD'),
    ('CA','Canada','en','Ottawa','Centretown','Bank Street','America/Toronto','CAD'),
    ('FI','Finland','fi','Helsinki','Kallio','Hämeentie','Europe/Helsinki','EUR'),
    ('DE','Germany','de','Berlin','Mitte','Friedrichstraße','Europe/Berlin','EUR'),
    ('IN','India','en','Bengaluru','Indiranagar','Market Road','Asia/Kolkata','USD'),
    ('IT','Italy','it','Milano','Brera','Via Solferino','Europe/Rome','EUR'),
    ('KR','South Korea','ko','Seoul','Mapo','World Cup Road','Asia/Seoul','KRW'),
    ('NG','Nigeria','en','Lagos','Ikeja','Allen Avenue','Africa/Lagos','USD'),
    ('PH','Philippines','en','Manila','Makati','Ayala Avenue','Asia/Manila','USD'),
    ('PL','Poland','pl','Warszawa','Śródmieście','Marszałkowska','Europe/Warsaw','EUR'),
    ('SG','Singapore','en','Singapore','Outram','Neil Road','Asia/Singapore','USD'),
    ('ZA','South Africa','en','Cape Town','Gardens','Kloof Street','Africa/Johannesburg','USD'),
    ('ES','Spain','es','Madrid','Centro','Calle Mayor','Europe/Madrid','EUR'),
    ('SE','Sweden','sv','Stockholm','Södermalm','Götgatan','Europe/Stockholm','EUR'),
    ('TH','Thailand','th','Bangkok','Watthana','Sukhumvit Road','Asia/Bangkok','USD'),
    ('TR','Türkiye','tr','Istanbul','Kadıköy','Bahariye Street','Europe/Istanbul','EUR'),
    ('GB','United Kingdom','en','London','Camden','High Street','Europe/London','GBP'),
    ('US','United States','en','Seattle','Fremont','Fremont Avenue','America/Los_Angeles','USD'),
]
TENANTS = [
    ('atlas-market','Atlas Market Network','retail','Maya Chen','partner'),
    ('harbor-services','Harbor Services Group','services','Noah Martin','self_service'),
    ('cedar-clinics','Cedar Clinic Partners','healthcare','Amara Okafor','partner'),
    ('northstar-supply','Northstar Supply Cooperative','manufacturing','Leo Berg','partner'),
    ('orchard-stores','Orchard Stores Collective','retail','Elena Rossi','self_service'),
    ('river-workshops','River Workshop Association','services','Arun Shah','partner'),
    ('lighthouse-care','Lighthouse Care Network','healthcare','Sofia Lind','self_service'),
    ('summit-makers','Summit Makers Alliance','manufacturing','Deniz Kaya','partner'),
    ('meadow-commerce','Meadow Commerce Partners','retail','Jiho Park','self_service'),
    ('horizon-professionals','Horizon Professional Services','services','Mina Santos','partner'),
]

def country(entity):
    if entity in ('ABA_ROUTING_NUMBER','MEDICAL_LICENSE'):
        return 'US'
    prefix = entity.split('_')[0]
    return 'GB' if prefix == 'UK' else prefix

grouped = {region[0]: [r['entity'] for r in catalog if country(r['entity']) == region[0]] for region in REGIONS}
international = [r['entity'] for r in catalog if r['entity'] not in {e for values in grouped.values() for e in values}]
assert len(international) == 9

def values_for(entity):
    result=[]
    for case in examples:
        if case['entity'] != entity or case['expected_len'] != 1:
            continue
        if case.get('ranges'):
            left,right=case['ranges'][0]
            value=case['text'].encode()[left:right].decode()
        else:
            value=case['text']
            if not re.fullmatch(r'[A-Z0-9 _./+-]{4,40}',value):
                continue
            if entity=='DE_PLZ':
                # A raw prose example can contain "PLZ 80331". The API field
                # stores the five-digit postcode, not the preceding prose label.
                match=re.fullmatch(r'(?:PLZ\s+)?([0-9]{5})',value)
                if not match:
                    continue
                value=match[1]
        if any(c in value for c in ('\n','\r','"',"'",'\\')) or not value or len(value)>80:
            continue
        if value not in result:
            result.append(value)
    assert result,entity
    return result

values={entity:values_for(entity) for entity in FIELDS}
