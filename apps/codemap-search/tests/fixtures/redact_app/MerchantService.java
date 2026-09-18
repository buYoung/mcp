// Offline onboarding, review, settlement and document export with synthetic inputs.
// Compile with javac --release 17 MerchantService.java; run java MerchantService.
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.StringJoiner;

public final class MerchantService {
    record Address(String city, String district, String streetName, String buildingNumber,
                   String unitName, String timeZone) {}
    record Business(String sector, String legalForm, int foundedYear, int employeeCount,
                    long annualTurnoverCents, long averageOrderCents,
                    boolean hasPhysicalStore, boolean hasOnlineStore) {}
    record Preferences(String language, String invoiceDelivery, String notificationMode,
                       boolean hasMarketingConsent, boolean hasServiceConsent) {}
    record Limits(long singlePaymentCents, long dailyPaymentCents, int refundWindowDays,
                  int settlementDelayDays) {}
    record Capabilities(boolean canAcceptPayments, boolean canIssueRefunds,
                        boolean canRequestPayouts, boolean shouldRequireSecondReviewer) {}
    record Tenant(String slug, String displayName, String defaultCurrency, String defaultLocale,
                  List<String> enabledCountries, String plan, int monthlyDocumentLimit,
                  boolean isExportEnabled) {}

    static final class Verification {
        String cardNumber, bitcoinAddress, openedAt, email, iban;
        String ipAddress, macAddress, website, correlationId;
        List<String> fields() {
            return List.of(cardNumber, bitcoinAddress, openedAt, email, iban,
                ipAddress, macAddress, website, correlationId);
        }
    }

    static final class MerchantInput {
        String externalReference, displayName, country, locale, currency, contactName, contactRole;
        DocumentSet identifiers;
        Address address;
        Business business;
        Preferences preferences;
        Limits limits;
        Capabilities capabilities;
        List<String> tags;
        String statementDescriptor, supportQueue, onboardingChannel;

        void validate() {
            require(externalReference != null && !externalReference.isBlank(), "reference required");
            require(displayName != null && !displayName.isBlank(), "display name required");
            require(preferences.hasServiceConsent(), "service consent required");
            require(business.employeeCount() > 0 && business.averageOrderCents() > 0, "invalid business");
            require(limits.singlePaymentCents() <= limits.dailyPaymentCents(), "invalid limits");
            require(!address.city().isBlank() && !address.timeZone().isBlank(), "address required");
            require(!identifiers.fields().isEmpty(), "documents required");
            require(identifiers.fields().values().stream().noneMatch(String::isBlank), "empty document");
            require(tags != null && !tags.isEmpty() && !supportQueue.isBlank(), "routing required");
        }
    }

    record Scenario(Tenant tenant, Verification verification, long paymentAmountCents,
                    long partialRefundCents, long payoutAmountCents,
                    int expectedMerchantCount, int expectedDocumentCount,
                    List<MerchantInput> merchants) {}
    enum Status { DRAFT, REVIEW, ACTIVE }

    static final class Merchant {
        final String reference, country, city;
        final Map<String, String> documents;
        final Limits limits;
        final Capabilities capabilities;
        final boolean hasMarketingConsent;
        final Set<String> reviewers = new HashSet<>();
        Status status = Status.DRAFT;

        Merchant(MerchantInput input) {
            reference = input.externalReference;
            country = input.country;
            city = input.address.city();
            documents = Map.copyOf(input.identifiers.fields());
            limits = input.limits;
            capabilities = input.capabilities;
            hasMarketingConsent = input.preferences.hasMarketingConsent();
        }
    }

    static final class Payment {
        final String merchantReference;
        final long amountCents;
        long refundedCents;
        boolean isCaptured;
        Payment(String merchantReference, long amountCents) {
            this.merchantReference = merchantReference;
            this.amountCents = amountCents;
        }
    }

    record LedgerEntry(String reference, String account, String kind, long amountCents) {}
    record Event(int sequence, String kind, String reference) {}
    record Command(String action, String reference, String merchantReference, String reviewer, long amountCents) {}
    record Response(boolean isSuccess, String error) {}

    static void require(boolean condition, String message) {
        if (!condition) throw new IllegalArgumentException(message);
    }

    static final class Application {
        final Tenant tenant;
        final Map<String, Merchant> merchants = new LinkedHashMap<>();
        final Map<String, Payment> payments = new HashMap<>();
        final List<LedgerEntry> ledger = new ArrayList<>();
        final List<Event> events = new ArrayList<>();
        List<String> verification = List.of();

        Application(Tenant tenant) { this.tenant = tenant; }

        void emit(String kind, String reference) {
            events.add(new Event(events.size() + 1, kind, reference));
        }

        void verify(Verification input) {
            require(input.fields().stream().noneMatch(String::isBlank), "missing verification field");
            verification = List.copyOf(input.fields());
        }

        void register(MerchantInput input) {
            input.validate();
            require(tenant.enabledCountries().contains(input.country), "unsupported country");
            require(!merchants.containsKey(input.externalReference), "duplicate merchant");
            int currentDocuments = merchants.values().stream().mapToInt(m -> m.documents.size()).sum();
            require(currentDocuments + input.identifiers.fields().size() <= tenant.monthlyDocumentLimit(), "document quota exceeded");
            merchants.put(input.externalReference, new Merchant(input));
            emit("merchant.registered", input.externalReference);
        }

        Merchant merchant(String reference) {
            Merchant merchant = merchants.get(reference);
            require(merchant != null, "unknown merchant");
            return merchant;
        }

        Payment payment(String reference) {
            Payment payment = payments.get(reference);
            require(payment != null, "unknown payment");
            return payment;
        }

        void submit(String reference) {
            Merchant merchant = merchant(reference);
            require(merchant.status == Status.DRAFT, "merchant not a draft");
            merchant.status = Status.REVIEW;
            emit("merchant.submitted", reference);
        }

        void approve(String reference, String reviewer) {
            Merchant merchant = merchant(reference);
            require(merchant.status == Status.REVIEW, "merchant not under review");
            require(!reviewer.isBlank() && !merchant.reviewers.contains(reviewer), "independent reviewer required");
            merchant.reviewers.add(reviewer);
            int required = merchant.capabilities.shouldRequireSecondReviewer() ? 2 : 1;
            if (merchant.reviewers.size() == required) merchant.status = Status.ACTIVE;
            emit("merchant.reviewed", reference);
        }

        void authorize(String reference, String merchantReference, long amountCents) {
            Payment previous = payments.get(reference);
            if (previous != null) {
                require(previous.merchantReference.equals(merchantReference) && previous.amountCents == amountCents, "idempotency conflict");
                return;
            }
            Merchant merchant = merchant(merchantReference);
            require(merchant.status == Status.ACTIVE && merchant.capabilities.canAcceptPayments(), "merchant cannot accept payments");
            require(amountCents > 0 && amountCents <= merchant.limits.singlePaymentCents(), "invalid amount");
            payments.put(reference, new Payment(merchantReference, amountCents));
            emit("payment.authorized", reference);
        }

        void post(String reference, String account, String kind, long amountCents) {
            ledger.add(new LedgerEntry(reference, account, kind, amountCents));
            ledger.add(new LedgerEntry(reference, "clearing", kind, -amountCents));
        }

        long balance(String account) {
            return ledger.stream().filter(entry -> entry.account().equals(account)).mapToLong(LedgerEntry::amountCents).sum();
        }

        void capture(String reference) {
            Payment payment = payment(reference);
            if (payment.isCaptured) return;
            post(reference, payment.merchantReference, "capture", payment.amountCents);
            payment.isCaptured = true;
            emit("payment.captured", reference);
        }

        void refund(String reference, long amountCents) {
            Payment payment = payment(reference);
            require(payment.isCaptured && merchant(payment.merchantReference).capabilities.canIssueRefunds(), "refund disabled");
            require(amountCents > 0 && amountCents <= payment.amountCents - payment.refundedCents
                && amountCents <= balance(payment.merchantReference), "refund exceeds funds");
            post(reference, payment.merchantReference, "refund", -amountCents);
            payment.refundedCents += amountCents;
            emit("payment.refunded", reference);
        }

        void payout(String reference, long amountCents) {
            Merchant merchant = merchant(reference);
            require(merchant.status == Status.ACTIVE && merchant.capabilities.canRequestPayouts(), "payout disabled");
            require(amountCents > 0 && amountCents <= balance(reference), "insufficient funds");
            post(reference, reference, "payout", -amountCents);
            emit("payout.completed", reference);
        }

        Response dispatch(Command command) {
            try {
                switch (command.action()) {
                    case "submit" -> submit(command.reference());
                    case "approve" -> approve(command.reference(), command.reviewer());
                    case "authorize" -> authorize(command.reference(), command.merchantReference(), command.amountCents());
                    case "capture" -> capture(command.reference());
                    case "refund" -> refund(command.reference(), command.amountCents());
                    case "payout" -> payout(command.reference(), command.amountCents());
                    default -> throw new IllegalArgumentException("unknown command");
                }
                return new Response(true, "");
            } catch (IllegalArgumentException error) {
                return new Response(false, error.getMessage());
            }
        }

        String exportDocuments() {
            require(tenant.isExportEnabled(), "export disabled");
            StringJoiner rows = new StringJoiner("\n");
            rows.add("reference,country,city,document,value");
            for (Merchant merchant : merchants.values()) {
                merchant.documents.entrySet().stream().sorted(Map.Entry.comparingByKey()).forEach(entry -> {
                    StringJoiner cells = new StringJoiner(",");
                    for (String cell : List.of(merchant.reference, merchant.country, merchant.city, entry.getKey(), entry.getValue())) {
                        cells.add("\"" + cell.replace("\"", "\"\"") + "\"");
                    }
                    rows.add(cells.toString());
                });
            }
            return rows.toString();
        }
    }

    static final class Checks {
        int assertions;
        void that(boolean condition, String message) {
            assertions++;
            if (!condition) throw new AssertionError(message);
        }
        void rejects(Runnable operation, String message) {
            boolean isRejected = false;
            try { operation.run(); } catch (IllegalArgumentException expected) { isRejected = true; }
            that(isRejected, message);
        }
        void command(Application app, Command command, boolean expected) {
            Response response = app.dispatch(command);
            that(response.isSuccess() == expected, command.action() + ": " + response.error());
        }
    }

    static int runScenario(Scenario scenario, Checks checks) {
        Application app = new Application(scenario.tenant());
        app.verify(scenario.verification());
        checks.that(app.verification.equals(scenario.verification().fields()), "verification changed");
        int documents = 0;
        for (MerchantInput input : scenario.merchants()) {
            String reference = input.externalReference;
            app.register(input);
            checks.rejects(() -> app.register(input), "duplicate merchant accepted");
            for (Map.Entry<String, String> field : input.identifiers.fields().entrySet()) {
                checks.that(app.merchant(reference).documents.get(field.getKey()).equals(field.getValue()), "stored document changed");
                documents++;
            }
            String payment = reference + "-payment";
            Command authorize = new Command("authorize", payment, reference, "", scenario.paymentAmountCents());
            checks.command(app, authorize, false);
            checks.command(app, new Command("submit", reference, "", "", 0), true);
            checks.command(app, new Command("approve", reference, "", "reviewer-a", 0), true);
            if (input.capabilities.shouldRequireSecondReviewer()) {
                checks.that(app.merchant(reference).status == Status.REVIEW, "second review bypassed");
                checks.command(app, new Command("approve", reference, "", "reviewer-a", 0), false);
                checks.command(app, new Command("approve", reference, "", "reviewer-b", 0), true);
            }
            checks.that(app.merchant(reference).status == Status.ACTIVE, "merchant not active");
            checks.command(app, authorize, true);
            checks.command(app, authorize, true);
            checks.command(app, new Command("authorize", payment, reference, "", scenario.paymentAmountCents() + 1), false);
            checks.command(app, new Command("capture", payment, "", "", 0), true);
            checks.command(app, new Command("capture", payment, "", "", 0), true);
            checks.that(app.balance(reference) == scenario.paymentAmountCents(), "capture duplicated");
            checks.command(app, new Command("refund", payment, "", "", scenario.partialRefundCents()), true);
            checks.command(app, new Command("payout", reference, "", "", scenario.payoutAmountCents()), true);
            long balance = scenario.paymentAmountCents() - scenario.partialRefundCents() - scenario.payoutAmountCents();
            checks.that(app.balance(reference) == balance, "balance mismatch");
            checks.command(app, new Command("payout", reference, "", "", balance + 1), false);
            checks.that(app.balance(reference) == balance, "failed payout changed balance");
        }
        checks.that(app.merchants.size() == scenario.expectedMerchantCount(), "missing merchants");
        checks.that(documents == scenario.expectedDocumentCount(), "missing documents");
        checks.that(app.ledger.stream().mapToLong(LedgerEntry::amountCents).sum() == 0, "ledger not balanced");
        String export = app.exportDocuments();
        checks.that(export.lines().count() == documents + 1, "export row count changed");
        for (MerchantInput input : scenario.merchants()) {
            for (String value : input.identifiers.fields().values()) checks.that(export.contains(value), "export changed document");
        }
        checks.that(app.merchants.values().stream().noneMatch(m -> m.hasMarketingConsent), "marketing consent changed");
        for (int index = 0; index < app.events.size(); index++) {
            checks.that(app.events.get(index).sequence() == index + 1, "event ordering changed");
        }
        checks.command(app, new Command("unknown", "", "", "", 0), false);
        return documents;
    }

    public static void main(String[] args) {
        int timeoutMs = 10000;
        int batchSize = 5000;
        String checksumAlgorithm = "sha256";
        Checks checks = new Checks();
        checks.that(timeoutMs > batchSize && checksumAlgorithm.equals("sha256"), "configuration changed");
        List<Scenario> scenarios = buildScenarios();
        int merchants = 0;
        int documents = 0;
        for (Scenario scenario : scenarios) {
            documents += runScenario(scenario, checks);
            merchants += scenario.merchants().size();
        }
        System.out.printf("{\"scenarios\":%d,\"merchants\":%d,\"documents\":%d,\"assertions\":%d}%n",
            scenarios.size(), merchants, documents, checks.assertions);
    }

    // BEGIN GENERATED SCENARIOS
    static final class DocumentSet {
        String aadhaarNumber;
        String abn;
        String acn;
        String bankAccount;
        String brn;
        String bsnr;
        String claimNumber;
        String companyRegistrationNumber;
        String deaNumber;
        String driverLicense;
        String drivingLicence;
        String drivingLicense;
        String fiscalCode;
        String frn;
        String gstin;
        String handelsregisternummer;
        String healthInsuranceNumber;
        String idNumber;
        String identityCardNumber;
        String incomeTaxNumber;
        String itinNumber;
        String lanr;
        String licensePlate;
        String mbiNumber;
        String medicareNumber;
        String memberId;
        String nationalIdNumber;
        String nhsNumber;
        String nie;
        String nif;
        String nin;
        String nino;
        String npiNumber;
        String nricFin;
        String organisationsnummer;
        String pan;
        String passportNumber;
        String personalIdentityCode;
        String personnummer;
        String pesel;
        String postalCode;
        String postcode;
        String prescriptionNumber;
        String priorAuthorizationNumber;
        String providerTaxId;
        String referralNumber;
        String routingNumber;
        String rrn;
        String sin;
        String socialSecurityNumber;
        String ssn;
        String taxId;
        String taxNumber;
        String tfn;
        String tin;
        String tnin;
        String trafficRegisterNumber;
        String uen;
        String umid;
        String vatCode;
        String vatId;
        String vatNumber;
        String vehicleRegistration;
        String voterId;
        Map<String, String> fields() {
            Map<String, String> fields = new LinkedHashMap<>();
            if (aadhaarNumber != null) fields.put("aadhaarNumber", aadhaarNumber);
            if (abn != null) fields.put("abn", abn);
            if (acn != null) fields.put("acn", acn);
            if (bankAccount != null) fields.put("bankAccount", bankAccount);
            if (brn != null) fields.put("brn", brn);
            if (bsnr != null) fields.put("bsnr", bsnr);
            if (claimNumber != null) fields.put("claimNumber", claimNumber);
            if (companyRegistrationNumber != null) fields.put("companyRegistrationNumber", companyRegistrationNumber);
            if (deaNumber != null) fields.put("deaNumber", deaNumber);
            if (driverLicense != null) fields.put("driverLicense", driverLicense);
            if (drivingLicence != null) fields.put("drivingLicence", drivingLicence);
            if (drivingLicense != null) fields.put("drivingLicense", drivingLicense);
            if (fiscalCode != null) fields.put("fiscalCode", fiscalCode);
            if (frn != null) fields.put("frn", frn);
            if (gstin != null) fields.put("gstin", gstin);
            if (handelsregisternummer != null) fields.put("handelsregisternummer", handelsregisternummer);
            if (healthInsuranceNumber != null) fields.put("healthInsuranceNumber", healthInsuranceNumber);
            if (idNumber != null) fields.put("idNumber", idNumber);
            if (identityCardNumber != null) fields.put("identityCardNumber", identityCardNumber);
            if (incomeTaxNumber != null) fields.put("incomeTaxNumber", incomeTaxNumber);
            if (itinNumber != null) fields.put("itinNumber", itinNumber);
            if (lanr != null) fields.put("lanr", lanr);
            if (licensePlate != null) fields.put("licensePlate", licensePlate);
            if (mbiNumber != null) fields.put("mbiNumber", mbiNumber);
            if (medicareNumber != null) fields.put("medicareNumber", medicareNumber);
            if (memberId != null) fields.put("memberId", memberId);
            if (nationalIdNumber != null) fields.put("nationalIdNumber", nationalIdNumber);
            if (nhsNumber != null) fields.put("nhsNumber", nhsNumber);
            if (nie != null) fields.put("nie", nie);
            if (nif != null) fields.put("nif", nif);
            if (nin != null) fields.put("nin", nin);
            if (nino != null) fields.put("nino", nino);
            if (npiNumber != null) fields.put("npiNumber", npiNumber);
            if (nricFin != null) fields.put("nricFin", nricFin);
            if (organisationsnummer != null) fields.put("organisationsnummer", organisationsnummer);
            if (pan != null) fields.put("pan", pan);
            if (passportNumber != null) fields.put("passportNumber", passportNumber);
            if (personalIdentityCode != null) fields.put("personalIdentityCode", personalIdentityCode);
            if (personnummer != null) fields.put("personnummer", personnummer);
            if (pesel != null) fields.put("pesel", pesel);
            if (postalCode != null) fields.put("postalCode", postalCode);
            if (postcode != null) fields.put("postcode", postcode);
            if (prescriptionNumber != null) fields.put("prescriptionNumber", prescriptionNumber);
            if (priorAuthorizationNumber != null) fields.put("priorAuthorizationNumber", priorAuthorizationNumber);
            if (providerTaxId != null) fields.put("providerTaxId", providerTaxId);
            if (referralNumber != null) fields.put("referralNumber", referralNumber);
            if (routingNumber != null) fields.put("routingNumber", routingNumber);
            if (rrn != null) fields.put("rrn", rrn);
            if (sin != null) fields.put("sin", sin);
            if (socialSecurityNumber != null) fields.put("socialSecurityNumber", socialSecurityNumber);
            if (ssn != null) fields.put("ssn", ssn);
            if (taxId != null) fields.put("taxId", taxId);
            if (taxNumber != null) fields.put("taxNumber", taxNumber);
            if (tfn != null) fields.put("tfn", tfn);
            if (tin != null) fields.put("tin", tin);
            if (tnin != null) fields.put("tnin", tnin);
            if (trafficRegisterNumber != null) fields.put("trafficRegisterNumber", trafficRegisterNumber);
            if (uen != null) fields.put("uen", uen);
            if (umid != null) fields.put("umid", umid);
            if (vatCode != null) fields.put("vatCode", vatCode);
            if (vatId != null) fields.put("vatId", vatId);
            if (vatNumber != null) fields.put("vatNumber", vatNumber);
            if (vehicleRegistration != null) fields.put("vehicleRegistration", vehicleRegistration);
            if (voterId != null) fields.put("voterId", voterId);
            return fields;
        }
    }

    static Scenario buildAtlasMarketScenario() {
        Tenant tenant = new Tenant(
            "atlas-market",
            "Atlas Market Network",
            "USD",
            "en",
            List.of("AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"),
            "standard",
            5000,
            true
        );
        Verification verification = new Verification();
        verification.cardNumber = "122000000000003";
        verification.bitcoinAddress = "16Yeky6GMjeNkAiNcBY7ZhrLoMSgg1BoyZ";
        verification.openedAt = "5-20-2021";
        verification.email = "info@presidio.site";
        verification.iban = "AL47212110090000000235698741";
        verification.ipAddress = "192.168.0.1";
        verification.macAddress = "00:1A:2B:3C:4D:5E";
        verification.website = "https://www.microsoft.com/";
        verification.correlationId = "550e8400-e29b-41d4-a716-446655440000";
        List<MerchantInput> merchants = new ArrayList<>();
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-au-branch";
            input.displayName = "Atlas Market Network Sydney";
            input.country = "AU";
            input.locale = "en-AU";
            input.currency = "AUD";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.abn = "51 824 753 556";
            input.identifiers.acn = "000 000 019";
            input.identifiers.medicareNumber = "2123 45670 1";
            input.identifiers.tfn = "876 543 210";
            input.address = new Address(
                "Sydney", "Surry Hills",
                "Crown Street", "12",
                "Main office", "Australia/Sydney"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 12,
                12500000, 2400,
                true, false
            );
            input.preferences = new Preferences(
                "en", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-ca-branch";
            input.displayName = "Atlas Market Network Ottawa";
            input.country = "CA";
            input.locale = "en-CA";
            input.currency = "CAD";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.postalCode = "K1A 0A1";
            input.identifiers.sin = "130 692 544";
            input.address = new Address(
                "Ottawa", "Centretown",
                "Bank Street", "12",
                "Main office", "America/Toronto"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 13,
                12500000, 2500,
                true, false
            );
            input.preferences = new Preferences(
                "en", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-fi-branch";
            input.displayName = "Atlas Market Network Helsinki";
            input.country = "FI";
            input.locale = "fi-FI";
            input.currency = "EUR";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.personalIdentityCode = "010594Y9032";
            input.address = new Address(
                "Helsinki", "Kallio",
                "Hämeentie", "12",
                "Main office", "Europe/Helsinki"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 14,
                12500000, 2600,
                true, false
            );
            input.preferences = new Preferences(
                "fi", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "fi-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-de-branch";
            input.displayName = "Atlas Market Network Berlin";
            input.country = "DE";
            input.locale = "de-DE";
            input.currency = "EUR";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.bsnr = "021234568";
            input.identifiers.drivingLicense = "BO12345678A";
            input.identifiers.handelsregisternummer = "HRB 123456";
            input.identifiers.healthInsuranceNumber = "A000500015";
            input.identifiers.identityCardNumber = "L01X00T44";
            input.identifiers.licensePlate = "B AB 1234";
            input.identifiers.lanr = "123456601";
            input.identifiers.passportNumber = "C01234565";
            input.identifiers.postalCode = "10115";
            input.identifiers.socialSecurityNumber = "15070649C103";
            input.identifiers.taxId = "12345678903";
            input.identifiers.taxNumber = "0281508150123";
            input.identifiers.vatId = "DE136695976";
            input.address = new Address(
                "Berlin", "Mitte",
                "Friedrichstraße", "12",
                "Main office", "Europe/Berlin"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 15,
                12500000, 2700,
                true, false
            );
            input.preferences = new Preferences(
                "de", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "de-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-in-branch";
            input.displayName = "Atlas Market Network Bengaluru";
            input.country = "IN";
            input.locale = "en-IN";
            input.currency = "USD";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.aadhaarNumber = "312345678909";
            input.identifiers.gstin = "27ABCDE1234F1Z5";
            input.identifiers.pan = "AAASA1111R";
            input.identifiers.passportNumber = "A3456781";
            input.identifiers.vehicleRegistration = "KA53ME3456";
            input.identifiers.voterId = "KSD1287349";
            input.address = new Address(
                "Bengaluru", "Indiranagar",
                "Market Road", "12",
                "Main office", "Asia/Kolkata"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 16,
                12500000, 2800,
                true, false
            );
            input.preferences = new Preferences(
                "en", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-it-branch";
            input.displayName = "Atlas Market Network Milano";
            input.country = "IT";
            input.locale = "it-IT";
            input.currency = "EUR";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.driverLicense = "AA0123456B";
            input.identifiers.fiscalCode = "AAAAAA00B11C333Y";
            input.identifiers.identityCardNumber = "1234567Aa";
            input.identifiers.passportNumber = "AA1234567";
            input.identifiers.vatCode = "01333550323";
            input.address = new Address(
                "Milano", "Brera",
                "Via Solferino", "12",
                "Main office", "Europe/Rome"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 17,
                12500000, 2900,
                true, false
            );
            input.preferences = new Preferences(
                "it", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "it-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-kr-branch";
            input.displayName = "Atlas Market Network Seoul";
            input.country = "KR";
            input.locale = "ko-KR";
            input.currency = "KRW";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.brn = "104-86-56659";
            input.identifiers.driverLicense = "11-22-123456-12";
            input.identifiers.frn = "911124-5678901";
            input.identifiers.passportNumber = "M123A4567";
            input.identifiers.rrn = "960121-1234567";
            input.address = new Address(
                "Seoul", "Mapo",
                "World Cup Road", "12",
                "Main office", "Asia/Seoul"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 18,
                12500000, 3000,
                true, false
            );
            input.preferences = new Preferences(
                "ko", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "ko-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-ng-branch";
            input.displayName = "Atlas Market Network Lagos";
            input.country = "NG";
            input.locale = "en-NG";
            input.currency = "USD";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nin = "12345678902";
            input.identifiers.vehicleRegistration = "APP-456CV";
            input.address = new Address(
                "Lagos", "Ikeja",
                "Allen Avenue", "12",
                "Main office", "Africa/Lagos"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 19,
                12500000, 3100,
                true, false
            );
            input.preferences = new Preferences(
                "en", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-ph-branch";
            input.displayName = "Atlas Market Network Manila";
            input.country = "PH";
            input.locale = "en-PH";
            input.currency = "USD";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.passportNumber = "P1234567A";
            input.identifiers.tin = "000-123-456-000";
            input.identifiers.umid = "0111-1234567-8";
            input.address = new Address(
                "Manila", "Makati",
                "Ayala Avenue", "12",
                "Main office", "Asia/Manila"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 20,
                12500000, 3200,
                true, false
            );
            input.preferences = new Preferences(
                "en", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-pl-branch";
            input.displayName = "Atlas Market Network Warszawa";
            input.country = "PL";
            input.locale = "pl-PL";
            input.currency = "EUR";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.pesel = "44051401458";
            input.address = new Address(
                "Warszawa", "Śródmieście",
                "Marszałkowska", "12",
                "Main office", "Europe/Warsaw"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 21,
                12500000, 3300,
                true, false
            );
            input.preferences = new Preferences(
                "pl", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "pl-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-sg-branch";
            input.displayName = "Atlas Market Network Singapore";
            input.country = "SG";
            input.locale = "en-SG";
            input.currency = "USD";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nricFin = "S2740116C";
            input.identifiers.uen = "53125226D";
            input.address = new Address(
                "Singapore", "Outram",
                "Neil Road", "12",
                "Main office", "Asia/Singapore"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 22,
                12500000, 3400,
                true, false
            );
            input.preferences = new Preferences(
                "en", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-za-branch";
            input.displayName = "Atlas Market Network Cape Town";
            input.country = "ZA";
            input.locale = "en-ZA";
            input.currency = "USD";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.companyRegistrationNumber = "2009/199240/23";
            input.identifiers.driverLicense = "60390002CGBV";
            input.identifiers.idNumber = "8001015009087";
            input.identifiers.incomeTaxNumber = "0123456789";
            input.identifiers.licensePlate = "KD93GKGP";
            input.identifiers.passportNumber = "A34855903";
            input.identifiers.trafficRegisterNumber = "1234567890123";
            input.identifiers.vatNumber = "4020269678";
            input.address = new Address(
                "Cape Town", "Gardens",
                "Kloof Street", "12",
                "Main office", "Africa/Johannesburg"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 23,
                12500000, 3500,
                true, false
            );
            input.preferences = new Preferences(
                "en", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-es-branch";
            input.displayName = "Atlas Market Network Madrid";
            input.country = "ES";
            input.locale = "es-ES";
            input.currency = "EUR";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nie = "Z8078221M";
            input.identifiers.nif = "55555555K";
            input.identifiers.passportNumber = "AAA123456";
            input.address = new Address(
                "Madrid", "Centro",
                "Calle Mayor", "12",
                "Main office", "Europe/Madrid"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 24,
                12500000, 3600,
                true, false
            );
            input.preferences = new Preferences(
                "es", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "es-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-se-branch";
            input.displayName = "Atlas Market Network Stockholm";
            input.country = "SE";
            input.locale = "sv-SE";
            input.currency = "EUR";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.organisationsnummer = "212000-0142";
            input.identifiers.personnummer = "189004119807";
            input.address = new Address(
                "Stockholm", "Södermalm",
                "Götgatan", "12",
                "Main office", "Europe/Stockholm"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 25,
                12500000, 3700,
                true, false
            );
            input.preferences = new Preferences(
                "sv", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "sv-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-th-branch";
            input.displayName = "Atlas Market Network Bangkok";
            input.country = "TH";
            input.locale = "th-TH";
            input.currency = "USD";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.tnin = "1234567890121";
            input.address = new Address(
                "Bangkok", "Watthana",
                "Sukhumvit Road", "12",
                "Main office", "Asia/Bangkok"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 26,
                12500000, 3800,
                true, false
            );
            input.preferences = new Preferences(
                "th", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "th-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-tr-branch";
            input.displayName = "Atlas Market Network Istanbul";
            input.country = "TR";
            input.locale = "tr-TR";
            input.currency = "EUR";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.licensePlate = "34 ABC 1234";
            input.identifiers.nationalIdNumber = "10000000146";
            input.address = new Address(
                "Istanbul", "Kadıköy",
                "Bahariye Street", "12",
                "Main office", "Europe/Istanbul"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 27,
                12500000, 3900,
                true, false
            );
            input.preferences = new Preferences(
                "tr", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "tr-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-gb-branch";
            input.displayName = "Atlas Market Network London";
            input.country = "GB";
            input.locale = "en-GB";
            input.currency = "GBP";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.drivingLicence = "MORGA607054SM9IJ";
            input.identifiers.nhsNumber = "401-023-2137";
            input.identifiers.nino = "AA 12 34 56 B";
            input.identifiers.passportNumber = "AB1234567";
            input.identifiers.postcode = "M1 1AA";
            input.identifiers.vehicleRegistration = "AB51 ABC";
            input.address = new Address(
                "London", "Camden",
                "High Street", "12",
                "Main office", "Europe/London"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 28,
                12500000, 4000,
                true, false
            );
            input.preferences = new Preferences(
                "en", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "atlas-market-us-branch";
            input.displayName = "Atlas Market Network Seattle";
            input.country = "US";
            input.locale = "en-US";
            input.currency = "USD";
            input.contactName = "Maya Chen";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.routingNumber = "121000358";
            input.identifiers.deaNumber = "K92993548";
            input.identifiers.bankAccount = "945456787654";
            input.identifiers.driverLicense = "H12234567";
            input.identifiers.memberId = "ABC123456789";
            input.identifiers.priorAuthorizationNumber = "PA-987654321";
            input.identifiers.claimNumber = "CLM456789123";
            input.identifiers.prescriptionNumber = "RX789456123";
            input.identifiers.referralNumber = "INF2025001234";
            input.identifiers.providerTaxId = "12-3456789";
            input.identifiers.itinNumber = "911701234";
            input.identifiers.mbiNumber = "1EG4-TE5-MK73";
            input.identifiers.npiNumber = "1234567893";
            input.identifiers.passportNumber = "912803456";
            input.identifiers.ssn = "078051121";
            input.address = new Address(
                "Seattle", "Fremont",
                "Fremont Avenue", "12",
                "Main office", "America/Los_Angeles"
            );
            input.business = new Business(
                "retail", "partnership",
                2005, 29,
                12500000, 4100,
                true, false
            );
            input.preferences = new Preferences(
                "en", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "partner", "standard");
            input.statementDescriptor = "atlas-market";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        return new Scenario(tenant, verification,
            18000, 1500, 5000, 18, 81, List.copyOf(merchants));
    }

    static Scenario buildHarborServicesScenario() {
        Tenant tenant = new Tenant(
            "harbor-services",
            "Harbor Services Group",
            "USD",
            "en",
            List.of("AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"),
            "enterprise",
            5000,
            true
        );
        Verification verification = new Verification();
        verification.cardNumber = "371449635398431";
        verification.bitcoinAddress = "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy";
        verification.openedAt = "5/20/2021";
        verification.email = "user@xn--80ak6aa92e.com";
        verification.iban = "AL47 2121 1009 0000 0002 3569 8741";
        verification.ipAddress = "684D:1111:222:3333:4444:5555:6:77";
        verification.macAddress = "AA:BB:CC:DD:EE:FF";
        verification.website = "http://www.microsoft.com/";
        verification.correlationId = "6fa459ea-ee8a-3ca4-894e-db77e160355e";
        List<MerchantInput> merchants = new ArrayList<>();
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-au-branch";
            input.displayName = "Harbor Services Group Sydney";
            input.country = "AU";
            input.locale = "en-AU";
            input.currency = "AUD";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.abn = "51824753556";
            input.identifiers.acn = "005 499 981";
            input.identifiers.medicareNumber = "2123456701";
            input.identifiers.tfn = "876543210";
            input.address = new Address(
                "Sydney", "Surry Hills",
                "Crown Street", "13",
                "Main office", "Australia/Sydney"
            );
            input.business = new Business(
                "services", "company",
                2006, 12,
                12600000, 2400,
                true, true
            );
            input.preferences = new Preferences(
                "en", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-ca-branch";
            input.displayName = "Harbor Services Group Ottawa";
            input.country = "CA";
            input.locale = "en-CA";
            input.currency = "CAD";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.postalCode = "K1A0A1";
            input.identifiers.sin = "435 418 165";
            input.address = new Address(
                "Ottawa", "Centretown",
                "Bank Street", "13",
                "Main office", "America/Toronto"
            );
            input.business = new Business(
                "services", "company",
                2006, 13,
                12600000, 2500,
                true, true
            );
            input.preferences = new Preferences(
                "en", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-fi-branch";
            input.displayName = "Harbor Services Group Helsinki";
            input.country = "FI";
            input.locale = "fi-FI";
            input.currency = "EUR";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.personalIdentityCode = "010594Y9021";
            input.address = new Address(
                "Helsinki", "Kallio",
                "Hämeentie", "13",
                "Main office", "Europe/Helsinki"
            );
            input.business = new Business(
                "services", "company",
                2006, 14,
                12600000, 2600,
                true, true
            );
            input.preferences = new Preferences(
                "fi", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "fi-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-de-branch";
            input.displayName = "Harbor Services Group Berlin";
            input.country = "DE";
            input.locale = "de-DE";
            input.currency = "EUR";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.bsnr = "521234567";
            input.identifiers.drivingLicense = "MU12345678B";
            input.identifiers.handelsregisternummer = "HRB 1";
            input.identifiers.healthInsuranceNumber = "C000500021";
            input.identifiers.identityCardNumber = "C01234565";
            input.identifiers.licensePlate = "M XY 999";
            input.identifiers.lanr = "234567701";
            input.identifiers.passportNumber = "F12345671";
            input.identifiers.postalCode = "80331";
            input.identifiers.socialSecurityNumber = "65070803A019";
            input.identifiers.taxId = "98765432106";
            input.identifiers.taxNumber = "0981508150999";
            input.identifiers.vatId = "DE129273398";
            input.address = new Address(
                "Berlin", "Mitte",
                "Friedrichstraße", "13",
                "Main office", "Europe/Berlin"
            );
            input.business = new Business(
                "services", "company",
                2006, 15,
                12600000, 2700,
                true, true
            );
            input.preferences = new Preferences(
                "de", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "de-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-in-branch";
            input.displayName = "Harbor Services Group Bengaluru";
            input.country = "IN";
            input.locale = "en-IN";
            input.currency = "USD";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.aadhaarNumber = "399876543211";
            input.identifiers.gstin = "07PQRST6789K1Z2";
            input.identifiers.pan = "ABCPD1234Z";
            input.identifiers.passportNumber = "B3097651";
            input.identifiers.vehicleRegistration = "KA99ME3456";
            input.identifiers.voterId = "KSD1287349";
            input.address = new Address(
                "Bengaluru", "Indiranagar",
                "Market Road", "13",
                "Main office", "Asia/Kolkata"
            );
            input.business = new Business(
                "services", "company",
                2006, 16,
                12600000, 2800,
                true, true
            );
            input.preferences = new Preferences(
                "en", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-it-branch";
            input.displayName = "Harbor Services Group Milano";
            input.country = "IT";
            input.locale = "it-IT";
            input.currency = "EUR";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.driverLicense = "U1H00B000C";
            input.identifiers.fiscalCode = "AAAAAA00B11C333N";
            input.identifiers.identityCardNumber = "AA12345aa";
            input.identifiers.passportNumber = "aa7654321";
            input.identifiers.vatCode = "01333550_323";
            input.address = new Address(
                "Milano", "Brera",
                "Via Solferino", "13",
                "Main office", "Europe/Rome"
            );
            input.business = new Business(
                "services", "company",
                2006, 17,
                12600000, 2900,
                true, true
            );
            input.preferences = new Preferences(
                "it", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "it-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-kr-branch";
            input.displayName = "Harbor Services Group Seoul";
            input.country = "KR";
            input.locale = "ko-KR";
            input.currency = "KRW";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.brn = "1048656659";
            input.identifiers.driverLicense = "112212345612";
            input.identifiers.frn = "9111245678901";
            input.identifiers.passportNumber = "m456B7890";
            input.identifiers.rrn = "9601211234567";
            input.address = new Address(
                "Seoul", "Mapo",
                "World Cup Road", "13",
                "Main office", "Asia/Seoul"
            );
            input.business = new Business(
                "services", "company",
                2006, 18,
                12600000, 3000,
                true, true
            );
            input.preferences = new Preferences(
                "ko", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "ko-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-ng-branch";
            input.displayName = "Harbor Services Group Lagos";
            input.country = "NG";
            input.locale = "en-NG";
            input.currency = "USD";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nin = "98765432102";
            input.identifiers.vehicleRegistration = "ABJ-001AA";
            input.address = new Address(
                "Lagos", "Ikeja",
                "Allen Avenue", "13",
                "Main office", "Africa/Lagos"
            );
            input.business = new Business(
                "services", "company",
                2006, 19,
                12600000, 3100,
                true, true
            );
            input.preferences = new Preferences(
                "en", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-ph-branch";
            input.displayName = "Harbor Services Group Manila";
            input.country = "PH";
            input.locale = "en-PH";
            input.currency = "USD";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.passportNumber = "Z0000000Z";
            input.identifiers.tin = "000123456";
            input.identifiers.umid = "0000-0000000-0";
            input.address = new Address(
                "Manila", "Makati",
                "Ayala Avenue", "13",
                "Main office", "Asia/Manila"
            );
            input.business = new Business(
                "services", "company",
                2006, 20,
                12600000, 3200,
                true, true
            );
            input.preferences = new Preferences(
                "en", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-pl-branch";
            input.displayName = "Harbor Services Group Warszawa";
            input.country = "PL";
            input.locale = "pl-PL";
            input.currency = "EUR";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.pesel = "02070803628";
            input.address = new Address(
                "Warszawa", "Śródmieście",
                "Marszałkowska", "13",
                "Main office", "Europe/Warsaw"
            );
            input.business = new Business(
                "services", "company",
                2006, 21,
                12600000, 3300,
                true, true
            );
            input.preferences = new Preferences(
                "pl", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "pl-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-sg-branch";
            input.displayName = "Harbor Services Group Singapore";
            input.country = "SG";
            input.locale = "en-SG";
            input.currency = "USD";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nricFin = "T1234567Z";
            input.identifiers.uen = "201434292D";
            input.address = new Address(
                "Singapore", "Outram",
                "Neil Road", "13",
                "Main office", "Asia/Singapore"
            );
            input.business = new Business(
                "services", "company",
                2006, 22,
                12600000, 3400,
                true, true
            );
            input.preferences = new Preferences(
                "en", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-za-branch";
            input.displayName = "Harbor Services Group Cape Town";
            input.country = "ZA";
            input.locale = "en-ZA";
            input.currency = "USD";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.companyRegistrationNumber = "2014/256030/07";
            input.identifiers.driverLicense = "4024048D4P60";
            input.identifiers.idNumber = "8001015000086";
            input.identifiers.incomeTaxNumber = "1234567890";
            input.identifiers.licensePlate = "PMG017GP";
            input.identifiers.passportNumber = "D12345678";
            input.identifiers.trafficRegisterNumber = "6001015000076";
            input.identifiers.vatNumber = "4170229407";
            input.address = new Address(
                "Cape Town", "Gardens",
                "Kloof Street", "13",
                "Main office", "Africa/Johannesburg"
            );
            input.business = new Business(
                "services", "company",
                2006, 23,
                12600000, 3500,
                true, true
            );
            input.preferences = new Preferences(
                "en", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-es-branch";
            input.displayName = "Harbor Services Group Madrid";
            input.country = "ES";
            input.locale = "es-ES";
            input.currency = "EUR";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nie = "X9613851N";
            input.identifiers.nif = "55555555-K";
            input.identifiers.passportNumber = "XYZ987654";
            input.address = new Address(
                "Madrid", "Centro",
                "Calle Mayor", "13",
                "Main office", "Europe/Madrid"
            );
            input.business = new Business(
                "services", "company",
                2006, 24,
                12600000, 3600,
                true, true
            );
            input.preferences = new Preferences(
                "es", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "es-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-se-branch";
            input.displayName = "Harbor Services Group Stockholm";
            input.country = "SE";
            input.locale = "sv-SE";
            input.currency = "EUR";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.organisationsnummer = "2120000142";
            input.identifiers.personnummer = "189110089811";
            input.address = new Address(
                "Stockholm", "Södermalm",
                "Götgatan", "13",
                "Main office", "Europe/Stockholm"
            );
            input.business = new Business(
                "services", "company",
                2006, 25,
                12600000, 3700,
                true, true
            );
            input.preferences = new Preferences(
                "sv", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "sv-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-th-branch";
            input.displayName = "Harbor Services Group Bangkok";
            input.country = "TH";
            input.locale = "th-TH";
            input.currency = "USD";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.tnin = "2345678901234";
            input.address = new Address(
                "Bangkok", "Watthana",
                "Sukhumvit Road", "13",
                "Main office", "Asia/Bangkok"
            );
            input.business = new Business(
                "services", "company",
                2006, 26,
                12600000, 3800,
                true, true
            );
            input.preferences = new Preferences(
                "th", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "th-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-tr-branch";
            input.displayName = "Harbor Services Group Istanbul";
            input.country = "TR";
            input.locale = "tr-TR";
            input.currency = "EUR";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.licensePlate = "06 A 123";
            input.identifiers.nationalIdNumber = "76543210794";
            input.address = new Address(
                "Istanbul", "Kadıköy",
                "Bahariye Street", "13",
                "Main office", "Europe/Istanbul"
            );
            input.business = new Business(
                "services", "company",
                2006, 27,
                12600000, 3900,
                true, true
            );
            input.preferences = new Preferences(
                "tr", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "tr-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-gb-branch";
            input.displayName = "Harbor Services Group London";
            input.country = "GB";
            input.locale = "en-GB";
            input.currency = "GBP";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.drivingLicence = "MORGA657054SM9IJ";
            input.identifiers.nhsNumber = "221 395 1837";
            input.identifiers.nino = "hh 01 02 03 d";
            input.identifiers.passportNumber = "XY9876543";
            input.identifiers.postcode = "M60 1NW";
            input.identifiers.vehicleRegistration = "BD62XYZ";
            input.address = new Address(
                "London", "Camden",
                "High Street", "13",
                "Main office", "Europe/London"
            );
            input.business = new Business(
                "services", "company",
                2006, 28,
                12600000, 4000,
                true, true
            );
            input.preferences = new Preferences(
                "en", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "harbor-services-us-branch";
            input.displayName = "Harbor Services Group Seattle";
            input.country = "US";
            input.locale = "en-US";
            input.currency = "USD";
            input.contactName = "Noah Martin";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.routingNumber = "3222-7162-7";
            input.identifiers.deaNumber = "BB1388568";
            input.identifiers.bankAccount = "945456787654";
            input.identifiers.driverLicense = "H12234567";
            input.identifiers.memberId = "ZX-987654321";
            input.identifiers.priorAuthorizationNumber = "pa-123456";
            input.identifiers.claimNumber = "clm123456";
            input.identifiers.prescriptionNumber = "rX123456";
            input.identifiers.referralNumber = "inf123456";
            input.identifiers.providerTaxId = "20-1234567";
            input.identifiers.itinNumber = "911-70-1234";
            input.identifiers.mbiNumber = "1EG4TE5MK73";
            input.identifiers.npiNumber = "1245319599";
            input.identifiers.passportNumber = "Z12803456";
            input.identifiers.ssn = "078-05-1123";
            input.address = new Address(
                "Seattle", "Fremont",
                "Fremont Avenue", "13",
                "Main office", "America/Los_Angeles"
            );
            input.business = new Business(
                "services", "company",
                2006, 29,
                12600000, 4100,
                true, true
            );
            input.preferences = new Preferences(
                "en", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("services", "self_service", "priority");
            input.statementDescriptor = "harbor-services";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        return new Scenario(tenant, verification,
            18100, 1600, 5000, 18, 81, List.copyOf(merchants));
    }

    static Scenario buildCedarClinicsScenario() {
        Tenant tenant = new Tenant(
            "cedar-clinics",
            "Cedar Clinic Partners",
            "USD",
            "en",
            List.of("AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"),
            "standard",
            5000,
            true
        );
        Verification verification = new Verification();
        verification.cardNumber = "5555555555554444";
        verification.bitcoinAddress = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq";
        verification.openedAt = "2021-05-21";
        verification.email = "a@xn--d1acufc.xn--p1ai";
        verification.iban = "AD1200012030200359100100";
        verification.ipAddress = "::";
        verification.macAddress = "01:23:45:67:89:AB";
        verification.website = "http://www.microsoft.com";
        verification.correlationId = "f47ac10b-58cc-1372-8567-0e02b2c3d479";
        List<MerchantInput> merchants = new ArrayList<>();
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-au-branch";
            input.displayName = "Cedar Clinic Partners Sydney";
            input.country = "AU";
            input.locale = "en-AU";
            input.currency = "AUD";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.abn = "51 824 753 556";
            input.identifiers.acn = "006249976";
            input.identifiers.medicareNumber = "2123 45670 1";
            input.identifiers.tfn = "876 543 210";
            input.address = new Address(
                "Sydney", "Surry Hills",
                "Crown Street", "14",
                "Main office", "Australia/Sydney"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 12,
                12700000, 2400,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-ca-branch";
            input.displayName = "Cedar Clinic Partners Ottawa";
            input.country = "CA";
            input.locale = "en-CA";
            input.currency = "CAD";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.postalCode = "k1a 0a1";
            input.identifiers.sin = "948 584 792";
            input.address = new Address(
                "Ottawa", "Centretown",
                "Bank Street", "14",
                "Main office", "America/Toronto"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 13,
                12700000, 2500,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-fi-branch";
            input.displayName = "Cedar Clinic Partners Helsinki";
            input.country = "FI";
            input.locale = "fi-FI";
            input.currency = "EUR";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.personalIdentityCode = "020594X903P";
            input.address = new Address(
                "Helsinki", "Kallio",
                "Hämeentie", "14",
                "Main office", "Europe/Helsinki"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 14,
                12700000, 2600,
                true, true
            );
            input.preferences = new Preferences(
                "fi", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "fi-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-de-branch";
            input.displayName = "Cedar Clinic Partners Berlin";
            input.country = "DE";
            input.locale = "de-DE";
            input.currency = "EUR";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.bsnr = "711234567";
            input.identifiers.drivingLicense = "HH98765432C";
            input.identifiers.handelsregisternummer = "HRB123456";
            input.identifiers.healthInsuranceNumber = "A123456780";
            input.identifiers.identityCardNumber = "CZ6311T03";
            input.identifiers.licensePlate = "HH AB 1234";
            input.identifiers.lanr = "100000601";
            input.identifiers.passportNumber = "L01X00T44";
            input.identifiers.postalCode = "22085";
            input.identifiers.socialSecurityNumber = "20151090B023";
            input.identifiers.taxId = "12345678903";
            input.identifiers.taxNumber = "1681508150001";
            input.identifiers.vatId = "DE123456788";
            input.address = new Address(
                "Berlin", "Mitte",
                "Friedrichstraße", "14",
                "Main office", "Europe/Berlin"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 15,
                12700000, 2700,
                true, true
            );
            input.preferences = new Preferences(
                "de", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "de-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-in-branch";
            input.displayName = "Cedar Clinic Partners Bengaluru";
            input.country = "IN";
            input.locale = "en-IN";
            input.currency = "USD";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.aadhaarNumber = "3123 4567 8909";
            input.identifiers.gstin = "01ABCDE1234F1Z5";
            input.identifiers.pan = "ABCND1234Z";
            input.identifiers.passportNumber = "C3590543";
            input.identifiers.vehicleRegistration = "MN2412";
            input.identifiers.voterId = "KSD1287349";
            input.address = new Address(
                "Bengaluru", "Indiranagar",
                "Market Road", "14",
                "Main office", "Asia/Kolkata"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 16,
                12700000, 2800,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-it-branch";
            input.displayName = "Cedar Clinic Partners Milano";
            input.country = "IT";
            input.locale = "it-IT";
            input.currency = "EUR";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.driverLicense = "U1K711J11M";
            input.identifiers.fiscalCode = "AAAAAA00B11C333Y";
            input.identifiers.identityCardNumber = "1234567Aa";
            input.identifiers.passportNumber = "AA1234567";
            input.identifiers.vatCode = "01333550323";
            input.address = new Address(
                "Milano", "Brera",
                "Via Solferino", "14",
                "Main office", "Europe/Rome"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 17,
                12700000, 2900,
                true, true
            );
            input.preferences = new Preferences(
                "it", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "it-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-kr-branch";
            input.displayName = "Cedar Clinic Partners Seoul";
            input.country = "KR";
            input.locale = "ko-KR";
            input.currency = "KRW";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.brn = "104-82-13138";
            input.identifiers.driverLicense = "13-22-123456-12";
            input.identifiers.frn = "000505-7637892";
            input.identifiers.passportNumber = "d789C1234";
            input.identifiers.rrn = "000505-3637892";
            input.address = new Address(
                "Seoul", "Mapo",
                "World Cup Road", "14",
                "Main office", "Asia/Seoul"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 18,
                12700000, 3000,
                true, true
            );
            input.preferences = new Preferences(
                "ko", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "ko-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-ng-branch";
            input.displayName = "Cedar Clinic Partners Lagos";
            input.country = "NG";
            input.locale = "en-NG";
            input.currency = "USD";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nin = "01234567895";
            input.identifiers.vehicleRegistration = "KJA-999PZ";
            input.address = new Address(
                "Lagos", "Ikeja",
                "Allen Avenue", "14",
                "Main office", "Africa/Lagos"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 19,
                12700000, 3100,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-ph-branch";
            input.displayName = "Cedar Clinic Partners Manila";
            input.country = "PH";
            input.locale = "en-PH";
            input.currency = "USD";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.passportNumber = "EB1234567";
            input.identifiers.tin = "000123456000";
            input.identifiers.umid = "001112345678";
            input.address = new Address(
                "Manila", "Makati",
                "Ayala Avenue", "14",
                "Main office", "Asia/Manila"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 20,
                12700000, 3200,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-pl-branch";
            input.displayName = "Cedar Clinic Partners Warszawa";
            input.country = "PL";
            input.locale = "pl-PL";
            input.currency = "EUR";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.pesel = "11111111116";
            input.address = new Address(
                "Warszawa", "Śródmieście",
                "Marszałkowska", "14",
                "Main office", "Europe/Warsaw"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 21,
                12700000, 3300,
                true, true
            );
            input.preferences = new Preferences(
                "pl", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "pl-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-sg-branch";
            input.displayName = "Cedar Clinic Partners Singapore";
            input.country = "SG";
            input.locale = "en-SG";
            input.currency = "USD";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nricFin = "F2346401L";
            input.identifiers.uen = "T16RF0037C";
            input.address = new Address(
                "Singapore", "Outram",
                "Neil Road", "14",
                "Main office", "Asia/Singapore"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 22,
                12700000, 3400,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-za-branch";
            input.displayName = "Cedar Clinic Partners Cape Town";
            input.country = "ZA";
            input.locale = "en-ZA";
            input.currency = "USD";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.companyRegistrationNumber = "2020/804826/07";
            input.identifiers.driverLicense = "30040008X6Z6";
            input.identifiers.idNumber = "9202201234088";
            input.identifiers.incomeTaxNumber = "9123456789";
            input.identifiers.licensePlate = "BJ47HRZN";
            input.identifiers.passportNumber = "M87654321";
            input.identifiers.trafficRegisterNumber = "1234567890123";
            input.identifiers.vatNumber = "4250281542";
            input.address = new Address(
                "Cape Town", "Gardens",
                "Kloof Street", "14",
                "Main office", "Africa/Johannesburg"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 23,
                12700000, 3500,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-es-branch";
            input.displayName = "Cedar Clinic Partners Madrid";
            input.country = "ES";
            input.locale = "es-ES";
            input.currency = "EUR";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nie = "Y8063915Z";
            input.identifiers.nif = "1111111-G";
            input.identifiers.passportNumber = "aaa123456";
            input.address = new Address(
                "Madrid", "Centro",
                "Calle Mayor", "14",
                "Main office", "Europe/Madrid"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 24,
                12700000, 3600,
                true, true
            );
            input.preferences = new Preferences(
                "es", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "es-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-se-branch";
            input.displayName = "Cedar Clinic Partners Stockholm";
            input.country = "SE";
            input.locale = "sv-SE";
            input.currency = "EUR";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.organisationsnummer = "556703-7485";
            input.identifiers.personnummer = "191005059801";
            input.address = new Address(
                "Stockholm", "Södermalm",
                "Götgatan", "14",
                "Main office", "Europe/Stockholm"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 25,
                12700000, 3700,
                true, true
            );
            input.preferences = new Preferences(
                "sv", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "sv-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-th-branch";
            input.displayName = "Cedar Clinic Partners Bangkok";
            input.country = "TH";
            input.locale = "th-TH";
            input.currency = "USD";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.tnin = "3456789012347";
            input.address = new Address(
                "Bangkok", "Watthana",
                "Sukhumvit Road", "14",
                "Main office", "Asia/Bangkok"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 26,
                12700000, 3800,
                true, true
            );
            input.preferences = new Preferences(
                "th", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "th-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-tr-branch";
            input.displayName = "Cedar Clinic Partners Istanbul";
            input.country = "TR";
            input.locale = "tr-TR";
            input.currency = "EUR";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.licensePlate = "35 JK 12";
            input.identifiers.nationalIdNumber = "36493665440";
            input.address = new Address(
                "Istanbul", "Kadıköy",
                "Bahariye Street", "14",
                "Main office", "Europe/Istanbul"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 27,
                12700000, 3900,
                true, true
            );
            input.preferences = new Preferences(
                "tr", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "tr-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-gb-branch";
            input.displayName = "Cedar Clinic Partners London";
            input.country = "GB";
            input.locale = "en-GB";
            input.currency = "GBP";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.drivingLicence = "FO999512018AA1AB";
            input.identifiers.nhsNumber = "0032698674";
            input.identifiers.nino = "tw987654a";
            input.identifiers.passportNumber = "ab1234567";
            input.identifiers.postcode = "W1A 1HQ";
            input.identifiers.vehicleRegistration = "LN14-HGT";
            input.address = new Address(
                "London", "Camden",
                "High Street", "14",
                "Main office", "Europe/London"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 28,
                12700000, 4000,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "cedar-clinics-us-branch";
            input.displayName = "Cedar Clinic Partners Seattle";
            input.country = "US";
            input.locale = "en-US";
            input.currency = "USD";
            input.contactName = "Amara Okafor";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.routingNumber = "121042882";
            input.identifiers.deaNumber = "K92993548";
            input.identifiers.bankAccount = "945456787654";
            input.identifiers.driverLicense = "H12234567";
            input.identifiers.memberId = "HPN12345A9";
            input.identifiers.priorAuthorizationNumber = "PA-123456";
            input.identifiers.claimNumber = "CLM123456";
            input.identifiers.prescriptionNumber = "RX123456";
            input.identifiers.referralNumber = "REF123456";
            input.identifiers.providerTaxId = "67-1234567";
            input.identifiers.itinNumber = "911-53-1234";
            input.identifiers.mbiNumber = "9XX9-XX9-XX99";
            input.identifiers.npiNumber = "1003000126";
            input.identifiers.passportNumber = "A12803456";
            input.identifiers.ssn = "078.05.1123";
            input.address = new Address(
                "Seattle", "Fremont",
                "Fremont Avenue", "14",
                "Main office", "America/Los_Angeles"
            );
            input.business = new Business(
                "healthcare", "partnership",
                2007, 29,
                12700000, 4100,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, true
            );
            input.tags = List.of("healthcare", "partner", "standard");
            input.statementDescriptor = "cedar-clinics";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        return new Scenario(tenant, verification,
            18200, 1700, 5000, 18, 81, List.copyOf(merchants));
    }

    static Scenario buildNorthstarSupplyScenario() {
        Tenant tenant = new Tenant(
            "northstar-supply",
            "Northstar Supply Cooperative",
            "USD",
            "en",
            List.of("AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"),
            "enterprise",
            5000,
            true
        );
        Verification verification = new Verification();
        verification.cardNumber = "5019717010103742";
        verification.bitcoinAddress = "bc1p5d7rjq7g6rdk2yhzks9smlaqtedr4dekq08ge8ztwac72sfr9rusxg3297";
        verification.openedAt = "21.5.2021";
        verification.email = "info@presidio.site";
        verification.iban = "AD12 0001 2030 2003 5910 0100";
        verification.ipAddress = "::1";
        verification.macAddress = "0a:23:f5:67:89:ac";
        verification.website = "http://microsoft.com";
        verification.correlationId = "550e8400-e29b-21d4-a716-446655440000";
        List<MerchantInput> merchants = new ArrayList<>();
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-au-branch";
            input.displayName = "Northstar Supply Cooperative Sydney";
            input.country = "AU";
            input.locale = "en-AU";
            input.currency = "AUD";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.abn = "51824753556";
            input.identifiers.acn = "000000180";
            input.identifiers.medicareNumber = "2123456701";
            input.identifiers.tfn = "876543210";
            input.address = new Address(
                "Sydney", "Surry Hills",
                "Crown Street", "15",
                "Main office", "Australia/Sydney"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 12,
                12800000, 2400,
                true, false
            );
            input.preferences = new Preferences(
                "en", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-ca-branch";
            input.displayName = "Northstar Supply Cooperative Ottawa";
            input.country = "CA";
            input.locale = "en-CA";
            input.currency = "CAD";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.postalCode = "K1a 0A1";
            input.identifiers.sin = "347-677-452";
            input.address = new Address(
                "Ottawa", "Centretown",
                "Bank Street", "15",
                "Main office", "America/Toronto"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 13,
                12800000, 2500,
                true, false
            );
            input.preferences = new Preferences(
                "en", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-fi-branch";
            input.displayName = "Northstar Supply Cooperative Helsinki";
            input.country = "FI";
            input.locale = "fi-FI";
            input.currency = "EUR";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.personalIdentityCode = "020594X902N";
            input.address = new Address(
                "Helsinki", "Kallio",
                "Hämeentie", "15",
                "Main office", "Europe/Helsinki"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 14,
                12800000, 2600,
                true, false
            );
            input.preferences = new Preferences(
                "fi", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "fi-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-de-branch";
            input.displayName = "Northstar Supply Cooperative Berlin";
            input.country = "DE";
            input.locale = "de-DE";
            input.currency = "EUR";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.bsnr = "351234567";
            input.identifiers.drivingLicense = "KO12345678X";
            input.identifiers.handelsregisternummer = "HRA 12345";
            input.identifiers.healthInsuranceNumber = "M123456785";
            input.identifiers.identityCardNumber = "G00000002";
            input.identifiers.licensePlate = "KA EF 12H";
            input.identifiers.lanr = "987654401";
            input.identifiers.passportNumber = "CZ6311T03";
            input.identifiers.postalCode = "01001";
            input.identifiers.socialSecurityNumber = "38551285K051";
            input.identifiers.taxId = "98765432106";
            input.identifiers.taxNumber = "0181508150000";
            input.identifiers.vatId = "DE111111117";
            input.address = new Address(
                "Berlin", "Mitte",
                "Friedrichstraße", "15",
                "Main office", "Europe/Berlin"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 15,
                12800000, 2700,
                true, false
            );
            input.preferences = new Preferences(
                "de", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "de-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-in-branch";
            input.displayName = "Northstar Supply Cooperative Bengaluru";
            input.country = "IN";
            input.locale = "en-IN";
            input.currency = "USD";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.aadhaarNumber = "3998 7654 3211";
            input.identifiers.gstin = "37ABCDE1234F1Z5";
            input.identifiers.pan = "A1111DFSFS";
            input.identifiers.passportNumber = "A3456781";
            input.identifiers.vehicleRegistration = "MCX1243";
            input.identifiers.voterId = "KSD1287349";
            input.address = new Address(
                "Bengaluru", "Indiranagar",
                "Market Road", "15",
                "Main office", "Asia/Kolkata"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 16,
                12800000, 2800,
                true, false
            );
            input.preferences = new Preferences(
                "en", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-it-branch";
            input.displayName = "Northstar Supply Cooperative Milano";
            input.country = "IT";
            input.locale = "it-IT";
            input.currency = "EUR";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.driverLicense = "AA0123456B";
            input.identifiers.fiscalCode = "AAAAAA00B11C333N";
            input.identifiers.identityCardNumber = "AA12345aa";
            input.identifiers.passportNumber = "aa7654321";
            input.identifiers.vatCode = "01333550_323";
            input.address = new Address(
                "Milano", "Brera",
                "Via Solferino", "15",
                "Main office", "Europe/Rome"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 17,
                12800000, 2900,
                true, false
            );
            input.preferences = new Preferences(
                "it", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "it-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-kr-branch";
            input.displayName = "Northstar Supply Cooperative Seoul";
            input.country = "KR";
            input.locale = "ko-KR";
            input.currency = "KRW";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.brn = "104-86-56659";
            input.identifiers.driverLicense = "28 22 123456 12";
            input.identifiers.frn = "0005056637892";
            input.identifiers.passportNumber = "S012D5678";
            input.identifiers.rrn = "0005053637892";
            input.address = new Address(
                "Seoul", "Mapo",
                "World Cup Road", "15",
                "Main office", "Asia/Seoul"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 18,
                12800000, 3000,
                true, false
            );
            input.preferences = new Preferences(
                "ko", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "ko-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-ng-branch";
            input.displayName = "Northstar Supply Cooperative Lagos";
            input.country = "NG";
            input.locale = "en-NG";
            input.currency = "USD";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nin = "12345678902";
            input.identifiers.vehicleRegistration = "APP 456CV";
            input.address = new Address(
                "Lagos", "Ikeja",
                "Allen Avenue", "15",
                "Main office", "Africa/Lagos"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 19,
                12800000, 3100,
                true, false
            );
            input.preferences = new Preferences(
                "en", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-ph-branch";
            input.displayName = "Northstar Supply Cooperative Manila";
            input.country = "PH";
            input.locale = "en-PH";
            input.currency = "USD";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.passportNumber = "AA0000000";
            input.identifiers.tin = "000-123-456-001";
            input.identifiers.umid = "1234-1234567-8";
            input.address = new Address(
                "Manila", "Makati",
                "Ayala Avenue", "15",
                "Main office", "Asia/Manila"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 20,
                12800000, 3200,
                true, false
            );
            input.preferences = new Preferences(
                "en", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-pl-branch";
            input.displayName = "Northstar Supply Cooperative Warszawa";
            input.country = "PL";
            input.locale = "pl-PL";
            input.currency = "EUR";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.pesel = "44051401458";
            input.address = new Address(
                "Warszawa", "Śródmieście",
                "Marszałkowska", "15",
                "Main office", "Europe/Warsaw"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 21,
                12800000, 3300,
                true, false
            );
            input.preferences = new Preferences(
                "pl", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "pl-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-sg-branch";
            input.displayName = "Northstar Supply Cooperative Singapore";
            input.country = "SG";
            input.locale = "en-SG";
            input.currency = "USD";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nricFin = "G1122144L";
            input.identifiers.uen = "S57TU0392K";
            input.address = new Address(
                "Singapore", "Outram",
                "Neil Road", "15",
                "Main office", "Asia/Singapore"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 22,
                12800000, 3400,
                true, false
            );
            input.preferences = new Preferences(
                "en", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-za-branch";
            input.displayName = "Northstar Supply Cooperative Cape Town";
            input.country = "ZA";
            input.locale = "en-ZA";
            input.currency = "USD";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.companyRegistrationNumber = "CK2001/123456";
            input.identifiers.driverLicense = "4046048YPC9T";
            input.identifiers.idNumber = "0002294321191";
            input.identifiers.incomeTaxNumber = "2987654321";
            input.identifiers.licensePlate = "DK 28 LF GP";
            input.identifiers.passportNumber = "T11223344";
            input.identifiers.trafficRegisterNumber = "6001015000076";
            input.identifiers.vatNumber = "4100168758";
            input.address = new Address(
                "Cape Town", "Gardens",
                "Kloof Street", "15",
                "Main office", "Africa/Johannesburg"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 23,
                12800000, 3500,
                true, false
            );
            input.preferences = new Preferences(
                "en", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-es-branch";
            input.displayName = "Northstar Supply Cooperative Madrid";
            input.country = "ES";
            input.locale = "es-ES";
            input.currency = "EUR";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nie = "Y8063915-Z";
            input.identifiers.nif = "1111111G";
            input.identifiers.passportNumber = "xyz987654";
            input.address = new Address(
                "Madrid", "Centro",
                "Calle Mayor", "15",
                "Main office", "Europe/Madrid"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 24,
                12800000, 3600,
                true, false
            );
            input.preferences = new Preferences(
                "es", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "es-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-se-branch";
            input.displayName = "Northstar Supply Cooperative Stockholm";
            input.country = "SE";
            input.locale = "sv-SE";
            input.currency = "EUR";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.organisationsnummer = "5567037485";
            input.identifiers.personnummer = "198712202384";
            input.address = new Address(
                "Stockholm", "Södermalm",
                "Götgatan", "15",
                "Main office", "Europe/Stockholm"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 25,
                12800000, 3700,
                true, false
            );
            input.preferences = new Preferences(
                "sv", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "sv-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-th-branch";
            input.displayName = "Northstar Supply Cooperative Bangkok";
            input.country = "TH";
            input.locale = "th-TH";
            input.currency = "USD";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.tnin = "4567890123459";
            input.address = new Address(
                "Bangkok", "Watthana",
                "Sukhumvit Road", "15",
                "Main office", "Asia/Bangkok"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 26,
                12800000, 3800,
                true, false
            );
            input.preferences = new Preferences(
                "th", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "th-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-tr-branch";
            input.displayName = "Northstar Supply Cooperative Istanbul";
            input.country = "TR";
            input.locale = "tr-TR";
            input.currency = "EUR";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.licensePlate = "16 B 1234";
            input.identifiers.nationalIdNumber = "53857632436";
            input.address = new Address(
                "Istanbul", "Kadıköy",
                "Bahariye Street", "15",
                "Main office", "Europe/Istanbul"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 27,
                12800000, 3900,
                true, false
            );
            input.preferences = new Preferences(
                "tr", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "tr-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-gb-branch";
            input.displayName = "Northstar Supply Cooperative London";
            input.country = "GB";
            input.locale = "en-GB";
            input.currency = "GBP";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.drivingLicence = "SMIT9801015JK2CD";
            input.identifiers.nhsNumber = "401-023-2137";
            input.identifiers.nino = "PR 123612C";
            input.identifiers.passportNumber = "CD7654321";
            input.identifiers.postcode = "CR2 6XH";
            input.identifiers.vehicleRegistration = "aa02 aaa";
            input.address = new Address(
                "London", "Camden",
                "High Street", "15",
                "Main office", "Europe/London"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 28,
                12800000, 4000,
                true, false
            );
            input.preferences = new Preferences(
                "en", "portal",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "northstar-supply-us-branch";
            input.displayName = "Northstar Supply Cooperative Seattle";
            input.country = "US";
            input.locale = "en-US";
            input.currency = "USD";
            input.contactName = "Leo Berg";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.routingNumber = "0711-0130-7";
            input.identifiers.deaNumber = "BB1388568";
            input.identifiers.bankAccount = "945456787654";
            input.identifiers.driverLicense = "H12234567";
            input.identifiers.memberId = "BCBSM1234567";
            input.identifiers.priorAuthorizationNumber = "PA-123456789012";
            input.identifiers.claimNumber = "CLM123456789012345";
            input.identifiers.prescriptionNumber = "RX123456789012";
            input.identifiers.referralNumber = "INF123456789012";
            input.identifiers.providerTaxId = "99-1234567";
            input.identifiers.itinNumber = "911-64-1234";
            input.identifiers.mbiNumber = "3CD5-FG7-HJ89";
            input.identifiers.npiNumber = "1234-567-893";
            input.identifiers.passportNumber = "912803456";
            input.identifiers.ssn = "078 05 1123";
            input.address = new Address(
                "Seattle", "Fremont",
                "Fremont Avenue", "15",
                "Main office", "America/Los_Angeles"
            );
            input.business = new Business(
                "manufacturing", "company",
                2008, 29,
                12800000, 4100,
                true, false
            );
            input.preferences = new Preferences(
                "en", "portal",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "northstar-supply";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        return new Scenario(tenant, verification,
            18300, 1800, 5000, 18, 81, List.copyOf(merchants));
    }

    static Scenario buildOrchardStoresScenario() {
        Tenant tenant = new Tenant(
            "orchard-stores",
            "Orchard Stores Collective",
            "USD",
            "en",
            List.of("AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"),
            "standard",
            5000,
            true
        );
        Verification verification = new Verification();
        verification.cardNumber = "30569309025904";
        verification.bitcoinAddress = "16Yeky6GMjeNkAiNcBY7ZhrLoMSgg1BoyZ";
        verification.openedAt = "21.5.21";
        verification.email = "user@xn--80ak6aa92e.com";
        verification.iban = "AT611904300234573201";
        verification.ipAddress = "2400:c401::5054:ff:fe1b:b031";
        verification.macAddress = "00-1A-2B-3C-4D-5E";
        verification.website = "http://microsoft.site";
        verification.correlationId = "6ba7b810-9dad-31d1-80b4-00c04fd430c8";
        List<MerchantInput> merchants = new ArrayList<>();
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-au-branch";
            input.displayName = "Orchard Stores Collective Sydney";
            input.country = "AU";
            input.locale = "en-AU";
            input.currency = "AUD";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.abn = "51 824 753 556";
            input.identifiers.acn = "000 000 019";
            input.identifiers.medicareNumber = "2123 45670 1";
            input.identifiers.tfn = "876 543 210";
            input.address = new Address(
                "Sydney", "Surry Hills",
                "Crown Street", "16",
                "Main office", "Australia/Sydney"
            );
            input.business = new Business(
                "retail", "partnership",
                2009, 12,
                12900000, 2400,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-ca-branch";
            input.displayName = "Orchard Stores Collective Ottawa";
            input.country = "CA";
            input.locale = "en-CA";
            input.currency = "CAD";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.postalCode = "K0A 0A1";
            input.identifiers.sin = "731-530-150";
            input.address = new Address(
                "Ottawa", "Centretown",
                "Bank Street", "16",
                "Main office", "America/Toronto"
            );
            input.business = new Business(
                "retail", "partnership",
                2009, 13,
                12900000, 2500,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-fi-branch";
            input.displayName = "Orchard Stores Collective Helsinki";
            input.country = "FI";
            input.locale = "fi-FI";
            input.currency = "EUR";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.personalIdentityCode = "030594W903B";
            input.address = new Address(
                "Helsinki", "Kallio",
                "Hämeentie", "16",
                "Main office", "Europe/Helsinki"
            );
            input.business = new Business(
                "retail", "partnership",
                2009, 14,
                12900000, 2600,
                true, true
            );
            input.preferences = new Preferences(
                "fi", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "fi-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-de-branch";
            input.displayName = "Orchard Stores Collective Berlin";
            input.country = "DE";
            input.locale = "de-DE";
            input.currency = "EUR";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.bsnr = "991234567";
            input.identifiers.drivingLicense = "DO98765432Z";
            input.identifiers.handelsregisternummer = "HRA12345";
            input.identifiers.healthInsuranceNumber = "B123456782";
            input.identifiers.identityCardNumber = "l01x00t44";
            input.identifiers.licensePlate = "S AB 12E";
            input.identifiers.lanr = "555555501";
            input.identifiers.passportNumber = "G00000002";
            input.identifiers.postalCode = "99998";
            input.identifiers.socialSecurityNumber = "15070649C103";
            input.identifiers.taxId = "12345678903";
            input.identifiers.taxNumber = "123/456/78901";
            input.identifiers.vatId = "DE123456789";
            input.address = new Address(
                "Berlin", "Mitte",
                "Friedrichstraße", "16",
                "Main office", "Europe/Berlin"
            );
            input.business = new Business(
                "retail", "partnership",
                2009, 15,
                12900000, 2700,
                true, true
            );
            input.preferences = new Preferences(
                "de", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "de-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-in-branch";
            input.displayName = "Orchard Stores Collective Bengaluru";
            input.country = "IN";
            input.locale = "en-IN";
            input.currency = "USD";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.aadhaarNumber = "3123-4567-8909";
            input.identifiers.gstin = "27ABCDE1234F1Z5";
            input.identifiers.pan = "AAASA1111R";
            input.identifiers.passportNumber = "B3097651";
            input.identifiers.vehicleRegistration = "I15432";
            input.identifiers.voterId = "KSD1287349";
            input.address = new Address(
                "Bengaluru", "Indiranagar",
                "Market Road", "16",
                "Main office", "Asia/Kolkata"
            );
            input.business = new Business(
                "retail", "partnership",
                2009, 16,
                12900000, 2800,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-it-branch";
            input.displayName = "Orchard Stores Collective Milano";
            input.country = "IT";
            input.locale = "it-IT";
            input.currency = "EUR";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.driverLicense = "U1H00B000C";
            input.identifiers.fiscalCode = "AAAAAA00B11C333Y";
            input.identifiers.identityCardNumber = "1234567Aa";
            input.identifiers.passportNumber = "AA1234567";
            input.identifiers.vatCode = "01333550323";
            input.address = new Address(
                "Milano", "Brera",
                "Via Solferino", "16",
                "Main office", "Europe/Rome"
            );
            input.business = new Business(
                "retail", "partnership",
                2009, 17,
                12900000, 2900,
                true, true
            );
            input.preferences = new Preferences(
                "it", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "it-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-kr-branch";
            input.displayName = "Orchard Stores Collective Seoul";
            input.country = "KR";
            input.locale = "ko-KR";
            input.currency = "KRW";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.brn = "1048656659";
            input.identifiers.driverLicense = "11-22-123456-12";
            input.identifiers.frn = "911124-5678906";
            input.identifiers.passportNumber = "M345E9012";
            input.identifiers.rrn = "960121-1021413";
            input.address = new Address(
                "Seoul", "Mapo",
                "World Cup Road", "16",
                "Main office", "Asia/Seoul"
            );
            input.business = new Business(
                "retail", "partnership",
                2009, 18,
                12900000, 3000,
                true, true
            );
            input.preferences = new Preferences(
                "ko", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "ko-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-ng-branch";
            input.displayName = "Orchard Stores Collective Lagos";
            input.country = "NG";
            input.locale = "en-NG";
            input.currency = "USD";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nin = "98765432102";
            input.identifiers.vehicleRegistration = "APP456CV";
            input.address = new Address(
                "Lagos", "Ikeja",
                "Allen Avenue", "16",
                "Main office", "Africa/Lagos"
            );
            input.business = new Business(
                "retail", "partnership",
                2009, 19,
                12900000, 3100,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-ph-branch";
            input.displayName = "Orchard Stores Collective Manila";
            input.country = "PH";
            input.locale = "en-PH";
            input.currency = "USD";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.passportNumber = "p1234567a";
            input.identifiers.tin = "000-123-456";
            input.identifiers.umid = "9999-9999999-9";
            input.address = new Address(
                "Manila", "Makati",
                "Ayala Avenue", "16",
                "Main office", "Asia/Manila"
            );
            input.business = new Business(
                "retail", "partnership",
                2009, 20,
                12900000, 3200,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-pl-branch";
            input.displayName = "Orchard Stores Collective Warszawa";
            input.country = "PL";
            input.locale = "pl-PL";
            input.currency = "EUR";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.pesel = "02070803628";
            input.address = new Address(
                "Warszawa", "Śródmieście",
                "Marszałkowska", "16",
                "Main office", "Europe/Warsaw"
            );
            input.business = new Business(
                "retail", "partnership",
                2009, 21,
                12900000, 3300,
                true, true
            );
            input.preferences = new Preferences(
                "pl", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "pl-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-sg-branch";
            input.displayName = "Orchard Stores Collective Singapore";
            input.country = "SG";
            input.locale = "en-SG";
            input.currency = "USD";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nricFin = "M4332674T";
            input.identifiers.uen = "R16RF0037F";
            input.address = new Address(
                "Singapore", "Outram",
                "Neil Road", "16",
                "Main office", "Asia/Singapore"
            );
            input.business = new Business(
                "retail", "partnership",
                2009, 22,
                12900000, 3400,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "immediate", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true, true,
                true, false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-za-branch";
            input.displayName = "Orchard Stores Collective Cape Town";
            input.country = "ZA";
            input.locale = "en-ZA";
            input.currency = "USD";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.companyRegistrationNumber = "CK1998/654321";
            input.identifiers.driverLicense = "114500482HFF";
            input.identifiers.idNumber = "9912316789285";
            input.identifiers.incomeTaxNumber = "0123456789";
            input.identifiers.licensePlate = "CC 75 CX ZN";
            input.identifiers.passportNumber = "A19299317";
            input.identifiers.trafficRegisterNumber = "1234567890123";
            input.identifiers.vatNumber = "4020269678";
            input.address = new Address(
                "Cape Town", "Gardens",
                "Kloof Street", "16",
                "Main office", "Africa/Johannesburg"
            );
            input.business = new Business(
                "retail", "partnership",
                2009, 23,
                12900000, 3500,
                true, true
            );
            input.preferences = new Preferences(
                "en", "email",
                "digest", false,
                true
            );
            input.limits = new Limits(
                50000, 250000,
                30, 2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-es-branch";
            input.displayName = "Orchard Stores Collective Madrid";
            input.country = "ES";
            input.locale = "es-ES";
            input.currency = "EUR";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nie = "x9613851n";
            input.identifiers.nif = "01111111G";
            input.identifiers.passportNumber = "AaA123456";
            input.address = new Address(
                "Madrid", "Centro",
                "Calle Mayor", "16",
                "Main office", "Europe/Madrid"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2009,
                24,
                12900000,
                3600,
                true,
                true
            );
            input.preferences = new Preferences(
                "es",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "es-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-se-branch";
            input.displayName = "Orchard Stores Collective Stockholm";
            input.country = "SE";
            input.locale = "sv-SE";
            input.currency = "EUR";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.organisationsnummer = "212000-0142";
            input.identifiers.personnummer = "871220-2384";
            input.address = new Address(
                "Stockholm",
                "Södermalm",
                "Götgatan",
                "16",
                "Main office",
                "Europe/Stockholm"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2009,
                25,
                12900000,
                3700,
                true,
                true
            );
            input.preferences = new Preferences(
                "sv",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "sv-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-th-branch";
            input.displayName = "Orchard Stores Collective Bangkok";
            input.country = "TH";
            input.locale = "th-TH";
            input.currency = "USD";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.tnin = "5678901234560";
            input.address = new Address(
                "Bangkok",
                "Watthana",
                "Sukhumvit Road",
                "16",
                "Main office",
                "Asia/Bangkok"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2009,
                26,
                12900000,
                3800,
                true,
                true
            );
            input.preferences = new Preferences(
                "th",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "th-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-tr-branch";
            input.displayName = "Orchard Stores Collective Istanbul";
            input.country = "TR";
            input.locale = "tr-TR";
            input.currency = "EUR";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.licensePlate = "34ABC1234";
            input.identifiers.nationalIdNumber = "94357219628";
            input.address = new Address(
                "Istanbul",
                "Kadıköy",
                "Bahariye Street",
                "16",
                "Main office",
                "Europe/Istanbul"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2009,
                27,
                12900000,
                3900,
                true,
                true
            );
            input.preferences = new Preferences(
                "tr",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "tr-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-gb-branch";
            input.displayName = "Orchard Stores Collective London";
            input.country = "GB";
            input.locale = "en-GB";
            input.currency = "GBP";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.drivingLicence = "morga607054sm9ij";
            input.identifiers.nhsNumber = "221 395 1837";
            input.identifiers.nino = "YZ 61 48 68 B";
            input.identifiers.passportNumber = "AB1234567";
            input.identifiers.postcode = "DN55 1PT";
            input.identifiers.vehicleRegistration = "AB70 DEF";
            input.address = new Address(
                "London",
                "Camden",
                "High Street",
                "16",
                "Main office",
                "Europe/London"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2009,
                28,
                12900000,
                4000,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "orchard-stores-us-branch";
            input.displayName = "Orchard Stores Collective Seattle";
            input.country = "US";
            input.locale = "en-US";
            input.currency = "USD";
            input.contactName = "Elena Rossi";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.routingNumber = "121000358";
            input.identifiers.deaNumber = "K92993548";
            input.identifiers.bankAccount = "945456787654";
            input.identifiers.driverLicense = "H12234567";
            input.identifiers.memberId = "UHC-12345AB";
            input.identifiers.priorAuthorizationNumber = "987654321";
            input.identifiers.claimNumber = "1234567890123";
            input.identifiers.prescriptionNumber = "1234567";
            input.identifiers.referralNumber = "2025001234";
            input.identifiers.providerTaxId = "12-3456789";
            input.identifiers.itinNumber = "911701234";
            input.identifiers.mbiNumber = "4EF6GH8JK12";
            input.identifiers.npiNumber = "1234 567 893";
            input.identifiers.passportNumber = "Z12803456";
            input.identifiers.ssn = "987-65-4321";
            input.address = new Address(
                "Seattle",
                "Fremont",
                "Fremont Avenue",
                "16",
                "Main office",
                "America/Los_Angeles"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2009,
                29,
                12900000,
                4100,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "orchard-stores";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        return new Scenario(tenant, verification,
            18400, 1900, 5000, 18, 81, List.copyOf(merchants));
    }

    static Scenario buildRiverWorkshopsScenario() {
        Tenant tenant = new Tenant(
            "river-workshops",
            "River Workshop Association",
            "USD",
            "en",
            List.of("AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"),
            "enterprise",
            5000,
            true
        );
        Verification verification = new Verification();
        verification.cardNumber = "6011000400000000";
        verification.bitcoinAddress = "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy";
        verification.openedAt = "5-MAY-2021";
        verification.email = "a@xn--d1acufc.xn--p1ai";
        verification.iban = "AT61 1904 3002 3457 3201";
        verification.ipAddress = "fe80::1";
        verification.macAddress = "AA-BB-CC-DD-EE-FF";
        verification.website = "http://microsoft.webcam";
        verification.correlationId = "74738ff5-5367-5958-9aee-98fffdcd1876";
        List<MerchantInput> merchants = new ArrayList<>();
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-au-branch";
            input.displayName = "River Workshop Association Sydney";
            input.country = "AU";
            input.locale = "en-AU";
            input.currency = "AUD";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.abn = "51824753556";
            input.identifiers.acn = "005 499 981";
            input.identifiers.medicareNumber = "2123456701";
            input.identifiers.tfn = "876543210";
            input.address = new Address(
                "Sydney",
                "Surry Hills",
                "Crown Street",
                "17",
                "Main office",
                "Australia/Sydney"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                12,
                13000000,
                2400,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-ca-branch";
            input.displayName = "River Workshop Association Ottawa";
            input.country = "CA";
            input.locale = "en-CA";
            input.currency = "CAD";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.postalCode = "K1A 1W1";
            input.identifiers.sin = "130692544";
            input.address = new Address(
                "Ottawa",
                "Centretown",
                "Bank Street",
                "17",
                "Main office",
                "America/Toronto"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                13,
                13000000,
                2500,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-fi-branch";
            input.displayName = "River Workshop Association Helsinki";
            input.country = "FI";
            input.locale = "fi-FI";
            input.currency = "EUR";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.personalIdentityCode = "030694W9024";
            input.address = new Address(
                "Helsinki",
                "Kallio",
                "Hämeentie",
                "17",
                "Main office",
                "Europe/Helsinki"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                14,
                13000000,
                2600,
                true,
                true
            );
            input.preferences = new Preferences(
                "fi",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "fi-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-de-branch";
            input.displayName = "River Workshop Association Berlin";
            input.country = "DE";
            input.locale = "de-DE";
            input.currency = "EUR";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.bsnr = "051234567";
            input.identifiers.drivingLicense = "GE123456780";
            input.identifiers.handelsregisternummer = "HRB 999999";
            input.identifiers.healthInsuranceNumber = "Z000000005";
            input.identifiers.identityCardNumber = "T22000129";
            input.identifiers.licensePlate = "MIL E 1234";
            input.identifiers.lanr = "999999901";
            input.identifiers.passportNumber = "C01X00T41";
            input.identifiers.postalCode = "10115";
            input.identifiers.socialSecurityNumber = "65070803A019";
            input.identifiers.taxId = "98765432106";
            input.identifiers.taxNumber = "987/654/32100";
            input.identifiers.vatId = "DE987654321";
            input.address = new Address(
                "Berlin",
                "Mitte",
                "Friedrichstraße",
                "17",
                "Main office",
                "Europe/Berlin"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                15,
                13000000,
                2700,
                true,
                true
            );
            input.preferences = new Preferences(
                "de",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "de-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-in-branch";
            input.displayName = "River Workshop Association Bengaluru";
            input.country = "IN";
            input.locale = "en-IN";
            input.currency = "USD";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.aadhaarNumber = "3998-7654-3211";
            input.identifiers.gstin = "07PQRST6789K1Z2";
            input.identifiers.pan = "ABCPD1234Z";
            input.identifiers.passportNumber = "C3590543";
            input.identifiers.vehicleRegistration = "DL3CJI0001";
            input.identifiers.voterId = "KSD1287349";
            input.address = new Address(
                "Bengaluru",
                "Indiranagar",
                "Market Road",
                "17",
                "Main office",
                "Asia/Kolkata"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                16,
                13000000,
                2800,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-it-branch";
            input.displayName = "River Workshop Association Milano";
            input.country = "IT";
            input.locale = "it-IT";
            input.currency = "EUR";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.driverLicense = "U1K711J11M";
            input.identifiers.fiscalCode = "AAAAAA00B11C333N";
            input.identifiers.identityCardNumber = "AA12345aa";
            input.identifiers.passportNumber = "aa7654321";
            input.identifiers.vatCode = "01333550_323";
            input.address = new Address(
                "Milano",
                "Brera",
                "Via Solferino",
                "17",
                "Main office",
                "Europe/Rome"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                17,
                13000000,
                2900,
                true,
                true
            );
            input.preferences = new Preferences(
                "it",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "it-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-kr-branch";
            input.displayName = "River Workshop Association Seoul";
            input.country = "KR";
            input.locale = "ko-KR";
            input.currency = "KRW";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.brn = "104-82-13138";
            input.identifiers.driverLicense = "112212345612";
            input.identifiers.frn = "9111245678906";
            input.identifiers.passportNumber = "M678f3456";
            input.identifiers.rrn = "9601211021413";
            input.address = new Address(
                "Seoul",
                "Mapo",
                "World Cup Road",
                "17",
                "Main office",
                "Asia/Seoul"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                18,
                13000000,
                3000,
                true,
                true
            );
            input.preferences = new Preferences(
                "ko",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "ko-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-ng-branch";
            input.displayName = "River Workshop Association Lagos";
            input.country = "NG";
            input.locale = "en-NG";
            input.currency = "USD";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nin = "01234567895";
            input.identifiers.vehicleRegistration = "ABJ-123XY";
            input.address = new Address(
                "Lagos",
                "Ikeja",
                "Allen Avenue",
                "17",
                "Main office",
                "Africa/Lagos"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                19,
                13000000,
                3100,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-ph-branch";
            input.displayName = "River Workshop Association Manila";
            input.country = "PH";
            input.locale = "en-PH";
            input.currency = "USD";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.passportNumber = "eb1234567";
            input.identifiers.tin = "000-123-456-000";
            input.identifiers.umid = "123456789012";
            input.address = new Address(
                "Manila",
                "Makati",
                "Ayala Avenue",
                "17",
                "Main office",
                "Asia/Manila"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                20,
                13000000,
                3200,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-pl-branch";
            input.displayName = "River Workshop Association Warszawa";
            input.country = "PL";
            input.locale = "pl-PL";
            input.currency = "EUR";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.pesel = "11111111116";
            input.address = new Address(
                "Warszawa",
                "Śródmieście",
                "Marszałkowska",
                "17",
                "Main office",
                "Europe/Warsaw"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                21,
                13000000,
                3300,
                true,
                true
            );
            input.preferences = new Preferences(
                "pl",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "pl-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-sg-branch";
            input.displayName = "River Workshop Association Singapore";
            input.country = "SG";
            input.locale = "en-SG";
            input.currency = "USD";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nricFin = "A1234567Z";
            input.identifiers.uen = "53125226d";
            input.address = new Address(
                "Singapore",
                "Outram",
                "Neil Road",
                "17",
                "Main office",
                "Asia/Singapore"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                22,
                13000000,
                3400,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-za-branch";
            input.displayName = "River Workshop Association Cape Town";
            input.country = "ZA";
            input.locale = "en-ZA";
            input.currency = "USD";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.companyRegistrationNumber = "2009/199240/23";
            input.identifiers.driverLicense = "40260039Y068";
            input.identifiers.idNumber = "0001015002288";
            input.identifiers.incomeTaxNumber = "1234567890";
            input.identifiers.licensePlate = "GET 103 WP";
            input.identifiers.passportNumber = "T99887766";
            input.identifiers.trafficRegisterNumber = "6001015000076";
            input.identifiers.vatNumber = "4170229407";
            input.address = new Address(
                "Cape Town",
                "Gardens",
                "Kloof Street",
                "17",
                "Main office",
                "Africa/Johannesburg"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                23,
                13000000,
                3500,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-es-branch";
            input.displayName = "River Workshop Association Madrid";
            input.country = "ES";
            input.locale = "es-ES";
            input.currency = "EUR";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nie = "z8078221m";
            input.identifiers.nif = "55555555k";
            input.identifiers.passportNumber = "XyZ987654";
            input.address = new Address(
                "Madrid",
                "Centro",
                "Calle Mayor",
                "17",
                "Main office",
                "Europe/Madrid"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                24,
                13000000,
                3600,
                true,
                true
            );
            input.preferences = new Preferences(
                "es",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "es-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-se-branch";
            input.displayName = "River Workshop Association Stockholm";
            input.country = "SE";
            input.locale = "sv-SE";
            input.currency = "EUR";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.organisationsnummer = "2120000142";
            input.identifiers.personnummer = "199109242397";
            input.address = new Address(
                "Stockholm",
                "Södermalm",
                "Götgatan",
                "17",
                "Main office",
                "Europe/Stockholm"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                25,
                13000000,
                3700,
                true,
                true
            );
            input.preferences = new Preferences(
                "sv",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "sv-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-th-branch";
            input.displayName = "River Workshop Association Bangkok";
            input.country = "TH";
            input.locale = "th-TH";
            input.currency = "USD";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.tnin = "1220000000007";
            input.address = new Address(
                "Bangkok",
                "Watthana",
                "Sukhumvit Road",
                "17",
                "Main office",
                "Asia/Bangkok"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                26,
                13000000,
                3800,
                true,
                true
            );
            input.preferences = new Preferences(
                "th",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "th-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-tr-branch";
            input.displayName = "River Workshop Association Istanbul";
            input.country = "TR";
            input.locale = "tr-TR";
            input.currency = "EUR";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.licensePlate = "34 abc 1234";
            input.identifiers.nationalIdNumber = "79059236630";
            input.address = new Address(
                "Istanbul",
                "Kadıköy",
                "Bahariye Street",
                "17",
                "Main office",
                "Europe/Istanbul"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                27,
                13000000,
                3900,
                true,
                true
            );
            input.preferences = new Preferences(
                "tr",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "tr-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-gb-branch";
            input.displayName = "River Workshop Association London";
            input.country = "GB";
            input.locale = "en-GB";
            input.currency = "GBP";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.drivingLicence = "JONES710153J99EF";
            input.identifiers.nhsNumber = "0032698674";
            input.identifiers.nino = "AB123456C";
            input.identifiers.passportNumber = "XY9876543";
            input.identifiers.postcode = "EC1A 1BB";
            input.identifiers.vehicleRegistration = "A123 BCD";
            input.address = new Address(
                "London",
                "Camden",
                "High Street",
                "17",
                "Main office",
                "Europe/London"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                28,
                13000000,
                4000,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "river-workshops-us-branch";
            input.displayName = "River Workshop Association Seattle";
            input.country = "US";
            input.locale = "en-US";
            input.currency = "USD";
            input.contactName = "Arun Shah";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.routingNumber = "3222-7162-7";
            input.identifiers.deaNumber = "BB1388568";
            input.identifiers.bankAccount = "945456787654";
            input.identifiers.driverLicense = "H12234567";
            input.identifiers.memberId = "AET987654";
            input.identifiers.priorAuthorizationNumber = "PA-987654321";
            input.identifiers.claimNumber = "123456789012345";
            input.identifiers.prescriptionNumber = "7654321";
            input.identifiers.referralNumber = "INF2025001234";
            input.identifiers.providerTaxId = "20-1234567";
            input.identifiers.itinNumber = "911-70-1234";
            input.identifiers.mbiNumber = "1eg4-te5-mk73";
            input.identifiers.npiNumber = "1234567893";
            input.identifiers.passportNumber = "A12803456";
            input.identifiers.ssn = "987-65-4322";
            input.address = new Address(
                "Seattle",
                "Fremont",
                "Fremont Avenue",
                "17",
                "Main office",
                "America/Los_Angeles"
            );
            input.business = new Business(
                "services",
                "company",
                2010,
                29,
                13000000,
                4100,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "river-workshops";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        return new Scenario(tenant, verification,
            18500, 2000, 5000, 18, 81, List.copyOf(merchants));
    }

    static Scenario buildLighthouseCareScenario() {
        Tenant tenant = new Tenant(
            "lighthouse-care",
            "Lighthouse Care Network",
            "USD",
            "en",
            List.of("AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"),
            "standard",
            5000,
            true
        );
        Verification verification = new Verification();
        verification.cardNumber = "3528000700000000";
        verification.bitcoinAddress = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq";
        verification.openedAt = "5-May-2021";
        verification.email = "info@presidio.site";
        verification.iban = "AZ21NABZ00000000137010001944";
        verification.ipAddress = "2001:db8::8a2e:370:7334";
        verification.macAddress = "01-23-45-67-89-AB";
        verification.website = "http://microsoft.vlaanderen";
        verification.correlationId = "1ec9414c-232a-6b00-b3c8-9e6bdeced846";
        List<MerchantInput> merchants = new ArrayList<>();
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-au-branch";
            input.displayName = "Lighthouse Care Network Sydney";
            input.country = "AU";
            input.locale = "en-AU";
            input.currency = "AUD";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.abn = "51 824 753 556";
            input.identifiers.acn = "006249976";
            input.identifiers.medicareNumber = "2123 45670 1";
            input.identifiers.tfn = "876 543 210";
            input.address = new Address(
                "Sydney",
                "Surry Hills",
                "Crown Street",
                "18",
                "Main office",
                "Australia/Sydney"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                12,
                13100000,
                2400,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-ca-branch";
            input.displayName = "Lighthouse Care Network Ottawa";
            input.country = "CA";
            input.locale = "en-CA";
            input.currency = "CAD";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.postalCode = "K1A 1Z1";
            input.identifiers.sin = "550090112";
            input.address = new Address(
                "Ottawa",
                "Centretown",
                "Bank Street",
                "18",
                "Main office",
                "America/Toronto"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                13,
                13100000,
                2500,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-fi-branch";
            input.displayName = "Lighthouse Care Network Helsinki";
            input.country = "FI";
            input.locale = "fi-FI";
            input.currency = "EUR";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.personalIdentityCode = "040594V9030";
            input.address = new Address(
                "Helsinki",
                "Kallio",
                "Hämeentie",
                "18",
                "Main office",
                "Europe/Helsinki"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                14,
                13100000,
                2600,
                true,
                false
            );
            input.preferences = new Preferences(
                "fi",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "fi-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-de-branch";
            input.displayName = "Lighthouse Care Network Berlin";
            input.country = "DE";
            input.locale = "de-DE";
            input.currency = "EUR";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.bsnr = "021234568";
            input.identifiers.drivingLicense = "MU123456785";
            input.identifiers.handelsregisternummer = "HRB 123456";
            input.identifiers.healthInsuranceNumber = "Z999999997";
            input.identifiers.identityCardNumber = "T00000000";
            input.identifiers.licensePlate = "MIL EF 1234E";
            input.identifiers.lanr = "123456601";
            input.identifiers.passportNumber = "c01234565";
            input.identifiers.postalCode = "80331";
            input.identifiers.socialSecurityNumber = "20151090B023";
            input.identifiers.taxId = "12345678903";
            input.identifiers.taxNumber = "12/345/6789";
            input.identifiers.vatId = "DE100000001";
            input.address = new Address(
                "Berlin",
                "Mitte",
                "Friedrichstraße",
                "18",
                "Main office",
                "Europe/Berlin"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                15,
                13100000,
                2700,
                true,
                false
            );
            input.preferences = new Preferences(
                "de",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "de-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-in-branch";
            input.displayName = "Lighthouse Care Network Bengaluru";
            input.country = "IN";
            input.locale = "en-IN";
            input.currency = "USD";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.aadhaarNumber = "312345678909";
            input.identifiers.gstin = "01ABCDE1234F1Z5";
            input.identifiers.pan = "ABCND1234Z";
            input.identifiers.passportNumber = "A3456781";
            input.identifiers.vehicleRegistration = "DL01CA1234";
            input.identifiers.voterId = "KSD1287349";
            input.address = new Address(
                "Bengaluru",
                "Indiranagar",
                "Market Road",
                "18",
                "Main office",
                "Asia/Kolkata"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                16,
                13100000,
                2800,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-it-branch";
            input.displayName = "Lighthouse Care Network Milano";
            input.country = "IT";
            input.locale = "it-IT";
            input.currency = "EUR";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.driverLicense = "AA0123456B";
            input.identifiers.fiscalCode = "AAAAAA00B11C333Y";
            input.identifiers.identityCardNumber = "1234567Aa";
            input.identifiers.passportNumber = "AA1234567";
            input.identifiers.vatCode = "01333550323";
            input.address = new Address(
                "Milano",
                "Brera",
                "Via Solferino",
                "18",
                "Main office",
                "Europe/Rome"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                17,
                13100000,
                2900,
                true,
                false
            );
            input.preferences = new Preferences(
                "it",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "it-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-kr-branch";
            input.displayName = "Lighthouse Care Network Seoul";
            input.country = "KR";
            input.locale = "ko-KR";
            input.currency = "KRW";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.brn = "104-86-56659";
            input.identifiers.driverLicense = "13-22-123456-12";
            input.identifiers.frn = "050912-6000012";
            input.identifiers.passportNumber = "M901g7890";
            input.identifiers.rrn = "050912-2000019";
            input.address = new Address(
                "Seoul",
                "Mapo",
                "World Cup Road",
                "18",
                "Main office",
                "Asia/Seoul"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                18,
                13100000,
                3000,
                true,
                false
            );
            input.preferences = new Preferences(
                "ko",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "ko-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-ng-branch";
            input.displayName = "Lighthouse Care Network Lagos";
            input.country = "NG";
            input.locale = "en-NG";
            input.currency = "USD";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nin = "12345678902";
            input.identifiers.vehicleRegistration = "app-456cv";
            input.address = new Address(
                "Lagos",
                "Ikeja",
                "Allen Avenue",
                "18",
                "Main office",
                "Africa/Lagos"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                19,
                13100000,
                3100,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-ph-branch";
            input.displayName = "Lighthouse Care Network Manila";
            input.country = "PH";
            input.locale = "en-PH";
            input.currency = "USD";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.passportNumber = "P1234567A";
            input.identifiers.tin = "000123456";
            input.identifiers.umid = "987654321098";
            input.address = new Address(
                "Manila",
                "Makati",
                "Ayala Avenue",
                "18",
                "Main office",
                "Asia/Manila"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                20,
                13100000,
                3200,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-pl-branch";
            input.displayName = "Lighthouse Care Network Warszawa";
            input.country = "PL";
            input.locale = "pl-PL";
            input.currency = "EUR";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.pesel = "44051401458";
            input.address = new Address(
                "Warszawa",
                "Śródmieście",
                "Marszałkowska",
                "18",
                "Main office",
                "Europe/Warsaw"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                21,
                13100000,
                3300,
                true,
                false
            );
            input.preferences = new Preferences(
                "pl",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "pl-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-sg-branch";
            input.displayName = "Lighthouse Care Network Singapore";
            input.country = "SG";
            input.locale = "en-SG";
            input.currency = "USD";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nricFin = "B1234567Z";
            input.identifiers.uen = "t16rf0037c";
            input.address = new Address(
                "Singapore",
                "Outram",
                "Neil Road",
                "18",
                "Main office",
                "Asia/Singapore"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                22,
                13100000,
                3400,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-za-branch";
            input.displayName = "Lighthouse Care Network Cape Town";
            input.country = "ZA";
            input.locale = "en-ZA";
            input.currency = "USD";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.companyRegistrationNumber = "2014/256030/07";
            input.identifiers.driverLicense = "60390002CGBV";
            input.identifiers.idNumber = "8001015009087";
            input.identifiers.incomeTaxNumber = "9123456789";
            input.identifiers.licensePlate = "015 SBZ EC";
            input.identifiers.passportNumber = "A34855903";
            input.identifiers.trafficRegisterNumber = "1234567890123";
            input.identifiers.vatNumber = "4250281542";
            input.address = new Address(
                "Cape Town",
                "Gardens",
                "Kloof Street",
                "18",
                "Main office",
                "Africa/Johannesburg"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                23,
                13100000,
                3500,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-es-branch";
            input.displayName = "Lighthouse Care Network Madrid";
            input.country = "ES";
            input.locale = "es-ES";
            input.currency = "EUR";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nie = "Z8078221M";
            input.identifiers.nif = "12345678z";
            input.identifiers.passportNumber = "AAA123456";
            input.address = new Address(
                "Madrid",
                "Centro",
                "Calle Mayor",
                "18",
                "Main office",
                "Europe/Madrid"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                24,
                13100000,
                3600,
                true,
                false
            );
            input.preferences = new Preferences(
                "es",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "es-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-se-branch";
            input.displayName = "Lighthouse Care Network Stockholm";
            input.country = "SE";
            input.locale = "sv-SE";
            input.currency = "EUR";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.organisationsnummer = "556703-7485";
            input.identifiers.personnummer = "19910924-2397";
            input.address = new Address(
                "Stockholm",
                "Södermalm",
                "Götgatan",
                "18",
                "Main office",
                "Europe/Stockholm"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                25,
                13100000,
                3700,
                true,
                false
            );
            input.preferences = new Preferences(
                "sv",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "sv-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-th-branch";
            input.displayName = "Lighthouse Care Network Bangkok";
            input.country = "TH";
            input.locale = "th-TH";
            input.currency = "USD";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.tnin = "1520000000004";
            input.address = new Address(
                "Bangkok",
                "Watthana",
                "Sukhumvit Road",
                "18",
                "Main office",
                "Asia/Bangkok"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                26,
                13100000,
                3800,
                true,
                false
            );
            input.preferences = new Preferences(
                "th",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "th-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-tr-branch";
            input.displayName = "Lighthouse Care Network Istanbul";
            input.country = "TR";
            input.locale = "tr-TR";
            input.currency = "EUR";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.licensePlate = "01 A 12";
            input.identifiers.nationalIdNumber = "64625294480";
            input.address = new Address(
                "Istanbul",
                "Kadıköy",
                "Bahariye Street",
                "18",
                "Main office",
                "Europe/Istanbul"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                27,
                13100000,
                3900,
                true,
                false
            );
            input.preferences = new Preferences(
                "tr",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "tr-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-gb-branch";
            input.displayName = "Lighthouse Care Network London";
            input.country = "GB";
            input.locale = "en-GB";
            input.currency = "GBP";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.drivingLicence = "SMITH802290AB1CD";
            input.identifiers.nhsNumber = "401-023-2137";
            input.identifiers.nino = "AB 12 34 56 C";
            input.identifiers.passportNumber = "ab1234567";
            input.identifiers.postcode = "GIR 0AA";
            input.identifiers.vehicleRegistration = "K1 ABC";
            input.address = new Address(
                "London",
                "Camden",
                "High Street",
                "18",
                "Main office",
                "Europe/London"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                28,
                13100000,
                4000,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "lighthouse-care-us-branch";
            input.displayName = "Lighthouse Care Network Seattle";
            input.country = "US";
            input.locale = "en-US";
            input.currency = "USD";
            input.contactName = "Sofia Lind";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.routingNumber = "121042882";
            input.identifiers.deaNumber = "K92993548";
            input.identifiers.bankAccount = "945456787654";
            input.identifiers.driverLicense = "H12234567";
            input.identifiers.memberId = "CIGNA123456";
            input.identifiers.priorAuthorizationNumber = "pa-123456";
            input.identifiers.claimNumber = "CLM456789123";
            input.identifiers.prescriptionNumber = "4455667";
            input.identifiers.referralNumber = "inf123456";
            input.identifiers.providerTaxId = "67-1234567";
            input.identifiers.itinNumber = "911-53-1234";
            input.identifiers.mbiNumber = "1EG4-TE5-MK73";
            input.identifiers.npiNumber = "1245319599";
            input.identifiers.passportNumber = "912803456";
            input.identifiers.ssn = "987-65-4323";
            input.address = new Address(
                "Seattle",
                "Fremont",
                "Fremont Avenue",
                "18",
                "Main office",
                "America/Los_Angeles"
            );
            input.business = new Business(
                "healthcare",
                "partnership",
                2011,
                29,
                13100000,
                4100,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                true
            );
            input.tags = List.of("healthcare", "self_service", "standard");
            input.statementDescriptor = "lighthouse-care";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        return new Scenario(tenant, verification,
            18600, 2100, 5000, 18, 81, List.copyOf(merchants));
    }

    static Scenario buildSummitMakersScenario() {
        Tenant tenant = new Tenant(
            "summit-makers",
            "Summit Makers Alliance",
            "USD",
            "en",
            List.of("AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"),
            "enterprise",
            5000,
            true
        );
        Verification verification = new Verification();
        verification.cardNumber = "6759649826438453";
        verification.bitcoinAddress = "bc1p5d7rjq7g6rdk2yhzks9smlaqtedr4dekq08ge8ztwac72sfr9rusxg3297";
        verification.openedAt = "05/21/21";
        verification.email = "user@xn--80ak6aa92e.com";
        verification.iban = "AZ21 NABZ 0000 0000 1370 1000 1944";
        verification.ipAddress = "2001:db8:85a3::8a2e:370";
        verification.macAddress = "01-b3-4a-67-d9-cf";
        verification.website = "https://webhook.site/a8eedfd6-9d8a-44e0-b0fc-cc7d517db5dc?q=1&b=2";
        verification.correlationId = "018f4f8e-9a3b-7c3d-8e9f-1a2b3c4d5e6f";
        List<MerchantInput> merchants = new ArrayList<>();
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-au-branch";
            input.displayName = "Summit Makers Alliance Sydney";
            input.country = "AU";
            input.locale = "en-AU";
            input.currency = "AUD";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.abn = "51824753556";
            input.identifiers.acn = "000000180";
            input.identifiers.medicareNumber = "2123456701";
            input.identifiers.tfn = "876543210";
            input.address = new Address(
                "Sydney",
                "Surry Hills",
                "Crown Street",
                "19",
                "Main office",
                "Australia/Sydney"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                12,
                13200000,
                2400,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-ca-branch";
            input.displayName = "Summit Makers Alliance Ottawa";
            input.country = "CA";
            input.locale = "en-CA";
            input.currency = "CAD";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.postalCode = "K1A 0A1";
            input.identifiers.sin = "130-692-544";
            input.address = new Address(
                "Ottawa",
                "Centretown",
                "Bank Street",
                "19",
                "Main office",
                "America/Toronto"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                13,
                13200000,
                2500,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-fi-branch";
            input.displayName = "Summit Makers Alliance Helsinki";
            input.country = "FI";
            input.locale = "fi-FI";
            input.currency = "EUR";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.personalIdentityCode = "040594V902Y";
            input.address = new Address(
                "Helsinki",
                "Kallio",
                "Hämeentie",
                "19",
                "Main office",
                "Europe/Helsinki"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                14,
                13200000,
                2600,
                true,
                true
            );
            input.preferences = new Preferences(
                "fi",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "fi-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-de-branch";
            input.displayName = "Summit Makers Alliance Berlin";
            input.country = "DE";
            input.locale = "de-DE";
            input.currency = "EUR";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.bsnr = "521234567";
            input.identifiers.drivingLicense = "BO12345678A";
            input.identifiers.handelsregisternummer = "HRB 1";
            input.identifiers.healthInsuranceNumber = "a123456780";
            input.identifiers.identityCardNumber = "T99999999";
            input.identifiers.licensePlate = "B-AB-1234";
            input.identifiers.lanr = "234567701";
            input.identifiers.passportNumber = "C01234565";
            input.identifiers.postalCode = "22085";
            input.identifiers.socialSecurityNumber = "38551285K051";
            input.identifiers.taxId = "98765432106";
            input.identifiers.taxNumber = "12/3456/7890";
            input.identifiers.vatId = "DE 136 695 976";
            input.address = new Address(
                "Berlin",
                "Mitte",
                "Friedrichstraße",
                "19",
                "Main office",
                "Europe/Berlin"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                15,
                13200000,
                2700,
                true,
                true
            );
            input.preferences = new Preferences(
                "de",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "de-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-in-branch";
            input.displayName = "Summit Makers Alliance Bengaluru";
            input.country = "IN";
            input.locale = "en-IN";
            input.currency = "USD";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.aadhaarNumber = "399876543211";
            input.identifiers.gstin = "37ABCDE1234F1Z5";
            input.identifiers.pan = "A1111DFSFS";
            input.identifiers.passportNumber = "B3097651";
            input.identifiers.vehicleRegistration = "GJ09AB1234";
            input.identifiers.voterId = "KSD1287349";
            input.address = new Address(
                "Bengaluru",
                "Indiranagar",
                "Market Road",
                "19",
                "Main office",
                "Asia/Kolkata"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                16,
                13200000,
                2800,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-it-branch";
            input.displayName = "Summit Makers Alliance Milano";
            input.country = "IT";
            input.locale = "it-IT";
            input.currency = "EUR";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.driverLicense = "U1H00B000C";
            input.identifiers.fiscalCode = "AAAAAA00B11C333N";
            input.identifiers.identityCardNumber = "AA12345aa";
            input.identifiers.passportNumber = "aa7654321";
            input.identifiers.vatCode = "01333550_323";
            input.address = new Address(
                "Milano",
                "Brera",
                "Via Solferino",
                "19",
                "Main office",
                "Europe/Rome"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                17,
                13200000,
                2900,
                true,
                true
            );
            input.preferences = new Preferences(
                "it",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "it-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-kr-branch";
            input.displayName = "Summit Makers Alliance Seoul";
            input.country = "KR";
            input.locale = "ko-KR";
            input.currency = "KRW";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.brn = "1048656659";
            input.identifiers.driverLicense = "28 22 123456 12";
            input.identifiers.frn = "0509126000012";
            input.identifiers.passportNumber = "M456B7890";
            input.identifiers.rrn = "0509122000019";
            input.address = new Address(
                "Seoul",
                "Mapo",
                "World Cup Road",
                "19",
                "Main office",
                "Asia/Seoul"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                18,
                13200000,
                3000,
                true,
                true
            );
            input.preferences = new Preferences(
                "ko",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "ko-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-ng-branch";
            input.displayName = "Summit Makers Alliance Lagos";
            input.country = "NG";
            input.locale = "en-NG";
            input.currency = "USD";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nin = "98765432102";
            input.identifiers.vehicleRegistration = "APP-456CV";
            input.address = new Address(
                "Lagos",
                "Ikeja",
                "Allen Avenue",
                "19",
                "Main office",
                "Africa/Lagos"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                19,
                13200000,
                3100,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-ph-branch";
            input.displayName = "Summit Makers Alliance Manila";
            input.country = "PH";
            input.locale = "en-PH";
            input.currency = "USD";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.passportNumber = "Z0000000Z";
            input.identifiers.tin = "000123456000";
            input.identifiers.umid = "0111-1234567-8";
            input.address = new Address(
                "Manila",
                "Makati",
                "Ayala Avenue",
                "19",
                "Main office",
                "Asia/Manila"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                20,
                13200000,
                3200,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-pl-branch";
            input.displayName = "Summit Makers Alliance Warszawa";
            input.country = "PL";
            input.locale = "pl-PL";
            input.currency = "EUR";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.pesel = "02070803628";
            input.address = new Address(
                "Warszawa",
                "Śródmieście",
                "Marszałkowska",
                "19",
                "Main office",
                "Europe/Warsaw"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                21,
                13200000,
                3300,
                true,
                true
            );
            input.preferences = new Preferences(
                "pl",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "pl-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-sg-branch";
            input.displayName = "Summit Makers Alliance Singapore";
            input.country = "SG";
            input.locale = "en-SG";
            input.currency = "USD";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nricFin = "S2740116C";
            input.identifiers.uen = "53125226D";
            input.address = new Address(
                "Singapore",
                "Outram",
                "Neil Road",
                "19",
                "Main office",
                "Asia/Singapore"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                22,
                13200000,
                3400,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-za-branch";
            input.displayName = "Summit Makers Alliance Cape Town";
            input.country = "ZA";
            input.locale = "en-ZA";
            input.currency = "USD";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.companyRegistrationNumber = "2020/804826/07";
            input.identifiers.driverLicense = "4024048D4P60";
            input.identifiers.idNumber = "8001015000086";
            input.identifiers.incomeTaxNumber = "2987654321";
            input.identifiers.licensePlate = "MT77GJGP";
            input.identifiers.passportNumber = "D12345678";
            input.identifiers.trafficRegisterNumber = "6001015000076";
            input.identifiers.vatNumber = "4100168758";
            input.address = new Address(
                "Cape Town",
                "Gardens",
                "Kloof Street",
                "19",
                "Main office",
                "Africa/Johannesburg"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                23,
                13200000,
                3500,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-es-branch";
            input.displayName = "Summit Makers Alliance Madrid";
            input.country = "ES";
            input.locale = "es-ES";
            input.currency = "EUR";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nie = "X9613851N";
            input.identifiers.nif = "12345678Z";
            input.identifiers.passportNumber = "XYZ987654";
            input.address = new Address(
                "Madrid",
                "Centro",
                "Calle Mayor",
                "19",
                "Main office",
                "Europe/Madrid"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                24,
                13200000,
                3600,
                true,
                true
            );
            input.preferences = new Preferences(
                "es",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "es-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-se-branch";
            input.displayName = "Summit Makers Alliance Stockholm";
            input.country = "SE";
            input.locale = "sv-SE";
            input.currency = "EUR";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.organisationsnummer = "5567037485";
            input.identifiers.personnummer = "199201232387";
            input.address = new Address(
                "Stockholm",
                "Södermalm",
                "Götgatan",
                "19",
                "Main office",
                "Europe/Stockholm"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                25,
                13200000,
                3700,
                true,
                true
            );
            input.preferences = new Preferences(
                "sv",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "sv-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-th-branch";
            input.displayName = "Summit Makers Alliance Bangkok";
            input.country = "TH";
            input.locale = "th-TH";
            input.currency = "USD";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.tnin = "1580000000004";
            input.address = new Address(
                "Bangkok",
                "Watthana",
                "Sukhumvit Road",
                "19",
                "Main office",
                "Asia/Bangkok"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                26,
                13200000,
                3800,
                true,
                true
            );
            input.preferences = new Preferences(
                "th",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "th-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-tr-branch";
            input.displayName = "Summit Makers Alliance Istanbul";
            input.country = "TR";
            input.locale = "tr-TR";
            input.currency = "EUR";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.licensePlate = "81 A 12";
            input.identifiers.nationalIdNumber = "10000000146";
            input.address = new Address(
                "Istanbul",
                "Kadıköy",
                "Bahariye Street",
                "19",
                "Main office",
                "Europe/Istanbul"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                27,
                13200000,
                3900,
                true,
                true
            );
            input.preferences = new Preferences(
                "tr",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "tr-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-gb-branch";
            input.displayName = "Summit Makers Alliance London";
            input.country = "GB";
            input.locale = "en-GB";
            input.currency = "GBP";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.drivingLicence = "SMITH812310AB1CD";
            input.identifiers.nhsNumber = "221 395 1837";
            input.identifiers.nino = "AA 12 34 56 B";
            input.identifiers.passportNumber = "CD7654321";
            input.identifiers.postcode = "M11AA";
            input.identifiers.vehicleRegistration = "M456DEF";
            input.address = new Address(
                "London",
                "Camden",
                "High Street",
                "19",
                "Main office",
                "Europe/London"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                28,
                13200000,
                4000,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "summit-makers-us-branch";
            input.displayName = "Summit Makers Alliance Seattle";
            input.country = "US";
            input.locale = "en-US";
            input.currency = "USD";
            input.contactName = "Deniz Kaya";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.routingNumber = "0711-0130-7";
            input.identifiers.deaNumber = "BB1388568";
            input.identifiers.bankAccount = "945456787654";
            input.identifiers.driverLicense = "H12234567";
            input.identifiers.memberId = "K123456789";
            input.identifiers.priorAuthorizationNumber = "PA-123456";
            input.identifiers.claimNumber = "clm123456";
            input.identifiers.prescriptionNumber = "RX789456123";
            input.identifiers.referralNumber = "REF123456";
            input.identifiers.providerTaxId = "99-1234567";
            input.identifiers.itinNumber = "911-64-1234";
            input.identifiers.mbiNumber = "1EG4TE5MK73";
            input.identifiers.npiNumber = "1003000126";
            input.identifiers.passportNumber = "Z12803456";
            input.identifiers.ssn = "987-65-4324";
            input.address = new Address(
                "Seattle",
                "Fremont",
                "Fremont Avenue",
                "19",
                "Main office",
                "America/Los_Angeles"
            );
            input.business = new Business(
                "manufacturing",
                "company",
                2012,
                29,
                13200000,
                4100,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("manufacturing", "partner", "priority");
            input.statementDescriptor = "summit-makers";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        return new Scenario(tenant, verification,
            18700, 2200, 5000, 18, 81, List.copyOf(merchants));
    }

    static Scenario buildMeadowCommerceScenario() {
        Tenant tenant = new Tenant(
            "meadow-commerce",
            "Meadow Commerce Partners",
            "USD",
            "en",
            List.of("AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"),
            "standard",
            5000,
            true
        );
        Verification verification = new Verification();
        verification.cardNumber = "4111111111111111";
        verification.bitcoinAddress = "16Yeky6GMjeNkAiNcBY7ZhrLoMSgg1BoyZ";
        verification.openedAt = "5/21/21";
        verification.email = "a@xn--d1acufc.xn--p1ai";
        verification.iban = "BH67BMAG00001299123456";
        verification.ipAddress = "2001:db8::1";
        verification.macAddress = "0d-B3-4a-6A-d9-cF";
        verification.website = "https://www.microsoft.com/store/abc/";
        verification.correlationId = "550e8400-e29b-81d4-a716-446655440000";
        List<MerchantInput> merchants = new ArrayList<>();
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-au-branch";
            input.displayName = "Meadow Commerce Partners Sydney";
            input.country = "AU";
            input.locale = "en-AU";
            input.currency = "AUD";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.abn = "51 824 753 556";
            input.identifiers.acn = "000 000 019";
            input.identifiers.medicareNumber = "2123 45670 1";
            input.identifiers.tfn = "876 543 210";
            input.address = new Address(
                "Sydney",
                "Surry Hills",
                "Crown Street",
                "20",
                "Main office",
                "Australia/Sydney"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                12,
                13300000,
                2400,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-ca-branch";
            input.displayName = "Meadow Commerce Partners Ottawa";
            input.country = "CA";
            input.locale = "en-CA";
            input.currency = "CAD";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.postalCode = "K1A0A1";
            input.identifiers.sin = "258 933 688";
            input.address = new Address(
                "Ottawa",
                "Centretown",
                "Bank Street",
                "20",
                "Main office",
                "America/Toronto"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                13,
                13300000,
                2500,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-fi-branch";
            input.displayName = "Meadow Commerce Partners Helsinki";
            input.country = "FI";
            input.locale = "fi-FI";
            input.currency = "EUR";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.personalIdentityCode = "050594U903M";
            input.address = new Address(
                "Helsinki",
                "Kallio",
                "Hämeentie",
                "20",
                "Main office",
                "Europe/Helsinki"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                14,
                13300000,
                2600,
                true,
                true
            );
            input.preferences = new Preferences(
                "fi",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "fi-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-de-branch";
            input.displayName = "Meadow Commerce Partners Berlin";
            input.country = "DE";
            input.locale = "de-DE";
            input.currency = "EUR";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.bsnr = "711234567";
            input.identifiers.drivingLicense = "MU12345678B";
            input.identifiers.handelsregisternummer = "HRB123456";
            input.identifiers.healthInsuranceNumber = "A000500015";
            input.identifiers.identityCardNumber = "t22000129";
            input.identifiers.licensePlate = "M-XY-999";
            input.identifiers.lanr = "100000601";
            input.identifiers.passportNumber = "F12345671";
            input.identifiers.postalCode = "01001";
            input.identifiers.socialSecurityNumber = "15070649C103";
            input.identifiers.taxId = "12345678903";
            input.identifiers.taxNumber = "123/3456/7890";
            input.identifiers.vatId = "DE 129 273 398";
            input.address = new Address(
                "Berlin",
                "Mitte",
                "Friedrichstraße",
                "20",
                "Main office",
                "Europe/Berlin"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                15,
                13300000,
                2700,
                true,
                true
            );
            input.preferences = new Preferences(
                "de",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "de-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-in-branch";
            input.displayName = "Meadow Commerce Partners Bengaluru";
            input.country = "IN";
            input.locale = "en-IN";
            input.currency = "USD";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.aadhaarNumber = "3123 4567 8909";
            input.identifiers.gstin = "27ABCDE1234F1Z5";
            input.identifiers.pan = "AAASA1111R";
            input.identifiers.passportNumber = "C3590543";
            input.identifiers.vehicleRegistration = "KA53ME3456";
            input.identifiers.voterId = "KSD1287349";
            input.address = new Address(
                "Bengaluru",
                "Indiranagar",
                "Market Road",
                "20",
                "Main office",
                "Asia/Kolkata"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                16,
                13300000,
                2800,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-it-branch";
            input.displayName = "Meadow Commerce Partners Milano";
            input.country = "IT";
            input.locale = "it-IT";
            input.currency = "EUR";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.driverLicense = "U1K711J11M";
            input.identifiers.fiscalCode = "AAAAAA00B11C333Y";
            input.identifiers.identityCardNumber = "1234567Aa";
            input.identifiers.passportNumber = "AA1234567";
            input.identifiers.vatCode = "01333550323";
            input.address = new Address(
                "Milano",
                "Brera",
                "Via Solferino",
                "20",
                "Main office",
                "Europe/Rome"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                17,
                13300000,
                2900,
                true,
                true
            );
            input.preferences = new Preferences(
                "it",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "it-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-kr-branch";
            input.displayName = "Meadow Commerce Partners Seoul";
            input.country = "KR";
            input.locale = "ko-KR";
            input.currency = "KRW";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.brn = "104-82-13138";
            input.identifiers.driverLicense = "11-22-123456-12";
            input.identifiers.frn = "911124-5678901";
            input.identifiers.passportNumber = "M789C1234";
            input.identifiers.rrn = "960121-1234567";
            input.address = new Address(
                "Seoul",
                "Mapo",
                "World Cup Road",
                "20",
                "Main office",
                "Asia/Seoul"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                18,
                13300000,
                3000,
                true,
                true
            );
            input.preferences = new Preferences(
                "ko",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "ko-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-ng-branch";
            input.displayName = "Meadow Commerce Partners Lagos";
            input.country = "NG";
            input.locale = "en-NG";
            input.currency = "USD";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nin = "01234567895";
            input.identifiers.vehicleRegistration = "ABJ-001AA";
            input.address = new Address(
                "Lagos",
                "Ikeja",
                "Allen Avenue",
                "20",
                "Main office",
                "Africa/Lagos"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                19,
                13300000,
                3100,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-ph-branch";
            input.displayName = "Meadow Commerce Partners Manila";
            input.country = "PH";
            input.locale = "en-PH";
            input.currency = "USD";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.passportNumber = "EB1234567";
            input.identifiers.tin = "000-123-456-001";
            input.identifiers.umid = "0000-0000000-0";
            input.address = new Address(
                "Manila",
                "Makati",
                "Ayala Avenue",
                "20",
                "Main office",
                "Asia/Manila"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                20,
                13300000,
                3200,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-pl-branch";
            input.displayName = "Meadow Commerce Partners Warszawa";
            input.country = "PL";
            input.locale = "pl-PL";
            input.currency = "EUR";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.pesel = "11111111116";
            input.address = new Address(
                "Warszawa",
                "Śródmieście",
                "Marszałkowska",
                "20",
                "Main office",
                "Europe/Warsaw"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                21,
                13300000,
                3300,
                true,
                true
            );
            input.preferences = new Preferences(
                "pl",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "pl-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-sg-branch";
            input.displayName = "Meadow Commerce Partners Singapore";
            input.country = "SG";
            input.locale = "en-SG";
            input.currency = "USD";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nricFin = "T1234567Z";
            input.identifiers.uen = "201434292D";
            input.address = new Address(
                "Singapore",
                "Outram",
                "Neil Road",
                "20",
                "Main office",
                "Asia/Singapore"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                22,
                13300000,
                3400,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-za-branch";
            input.displayName = "Meadow Commerce Partners Cape Town";
            input.country = "ZA";
            input.locale = "en-ZA";
            input.currency = "USD";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.companyRegistrationNumber = "CK2001/123456";
            input.identifiers.driverLicense = "30040008X6Z6";
            input.identifiers.idNumber = "9202201234088";
            input.identifiers.incomeTaxNumber = "0123456789";
            input.identifiers.licensePlate = "KD93GKGP";
            input.identifiers.passportNumber = "M87654321";
            input.identifiers.trafficRegisterNumber = "1234567890123";
            input.identifiers.vatNumber = "4020269678";
            input.address = new Address(
                "Cape Town",
                "Gardens",
                "Kloof Street",
                "20",
                "Main office",
                "Africa/Johannesburg"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                23,
                13300000,
                3500,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-es-branch";
            input.displayName = "Meadow Commerce Partners Madrid";
            input.country = "ES";
            input.locale = "es-ES";
            input.currency = "EUR";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nie = "Y8063915Z";
            input.identifiers.nif = "55555555K";
            input.identifiers.passportNumber = "aaa123456";
            input.address = new Address(
                "Madrid",
                "Centro",
                "Calle Mayor",
                "20",
                "Main office",
                "Europe/Madrid"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                24,
                13300000,
                3600,
                true,
                true
            );
            input.preferences = new Preferences(
                "es",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "es-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-se-branch";
            input.displayName = "Meadow Commerce Partners Stockholm";
            input.country = "SE";
            input.locale = "sv-SE";
            input.currency = "EUR";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.organisationsnummer = "212000-0142";
            input.identifiers.personnummer = "9201232387";
            input.address = new Address(
                "Stockholm",
                "Södermalm",
                "Götgatan",
                "20",
                "Main office",
                "Europe/Stockholm"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                25,
                13300000,
                3700,
                true,
                true
            );
            input.preferences = new Preferences(
                "sv",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "sv-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-th-branch";
            input.displayName = "Meadow Commerce Partners Bangkok";
            input.country = "TH";
            input.locale = "th-TH";
            input.currency = "USD";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.tnin = "1234567890121";
            input.address = new Address(
                "Bangkok",
                "Watthana",
                "Sukhumvit Road",
                "20",
                "Main office",
                "Asia/Bangkok"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                26,
                13300000,
                3800,
                true,
                true
            );
            input.preferences = new Preferences(
                "th",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "th-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-tr-branch";
            input.displayName = "Meadow Commerce Partners Istanbul";
            input.country = "TR";
            input.locale = "tr-TR";
            input.currency = "EUR";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.licensePlate = "07 AB 123";
            input.identifiers.nationalIdNumber = "76543210794";
            input.address = new Address(
                "Istanbul",
                "Kadıköy",
                "Bahariye Street",
                "20",
                "Main office",
                "Europe/Istanbul"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                27,
                13300000,
                3900,
                true,
                true
            );
            input.preferences = new Preferences(
                "tr",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "tr-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-gb-branch";
            input.displayName = "Meadow Commerce Partners London";
            input.country = "GB";
            input.locale = "en-GB";
            input.currency = "GBP";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.drivingLicence = "SMITH851010AB1CD";
            input.identifiers.nhsNumber = "0032698674";
            input.identifiers.nino = "hh 01 02 03 d";
            input.identifiers.passportNumber = "AB1234567";
            input.identifiers.postcode = "EC1A1BB";
            input.identifiers.vehicleRegistration = "ABC 123D";
            input.address = new Address(
                "London",
                "Camden",
                "High Street",
                "20",
                "Main office",
                "Europe/London"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                28,
                13300000,
                4000,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "meadow-commerce-us-branch";
            input.displayName = "Meadow Commerce Partners Seattle";
            input.country = "US";
            input.locale = "en-US";
            input.currency = "USD";
            input.contactName = "Jiho Park";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.routingNumber = "121000358";
            input.identifiers.deaNumber = "K92993548";
            input.identifiers.bankAccount = "945456787654";
            input.identifiers.driverLicense = "H12234567";
            input.identifiers.memberId = "abc123456";
            input.identifiers.priorAuthorizationNumber = "PA-123456789012";
            input.identifiers.claimNumber = "CLM123456";
            input.identifiers.prescriptionNumber = "rX123456";
            input.identifiers.referralNumber = "INF123456789012";
            input.identifiers.providerTaxId = "12-3456789";
            input.identifiers.itinNumber = "911701234";
            input.identifiers.mbiNumber = "9XX9-XX9-XX99";
            input.identifiers.npiNumber = "1234-567-893";
            input.identifiers.passportNumber = "A12803456";
            input.identifiers.ssn = "987-65-4325";
            input.address = new Address(
                "Seattle",
                "Fremont",
                "Fremont Avenue",
                "20",
                "Main office",
                "America/Los_Angeles"
            );
            input.business = new Business(
                "retail",
                "partnership",
                2013,
                29,
                13300000,
                4100,
                true,
                true
            );
            input.preferences = new Preferences(
                "en",
                "email",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("retail", "self_service", "standard");
            input.statementDescriptor = "meadow-commerce";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "self_service";
            merchants.add(input);
        }
        return new Scenario(tenant, verification,
            18800, 2300, 5000, 18, 81, List.copyOf(merchants));
    }

    static Scenario buildHorizonProfessionalsScenario() {
        Tenant tenant = new Tenant(
            "horizon-professionals",
            "Horizon Professional Services",
            "USD",
            "en",
            List.of("AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"),
            "enterprise",
            5000,
            true
        );
        Verification verification = new Verification();
        verification.cardNumber = "4917300800000000";
        verification.bitcoinAddress = "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy";
        verification.openedAt = "21/05/21";
        verification.email = "info@presidio.site";
        verification.iban = "BH67 BMAG 0000 1299 1234 56";
        verification.ipAddress = "fe80::1%eth0";
        verification.macAddress = "0012.3456.789A";
        verification.website = "microsoft.com";
        verification.correlationId = "550E8400-E29B-41D4-A716-446655440000";
        List<MerchantInput> merchants = new ArrayList<>();
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-au-branch";
            input.displayName = "Horizon Professional Services Sydney";
            input.country = "AU";
            input.locale = "en-AU";
            input.currency = "AUD";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.abn = "51824753556";
            input.identifiers.acn = "005 499 981";
            input.identifiers.medicareNumber = "2123456701";
            input.identifiers.tfn = "876543210";
            input.address = new Address(
                "Sydney",
                "Surry Hills",
                "Crown Street",
                "21",
                "Main office",
                "Australia/Sydney"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                12,
                13400000,
                2400,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-ca-branch";
            input.displayName = "Horizon Professional Services Ottawa";
            input.country = "CA";
            input.locale = "en-CA";
            input.currency = "CAD";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.postalCode = "k1a 0a1";
            input.identifiers.sin = "130 692 544";
            input.address = new Address(
                "Ottawa",
                "Centretown",
                "Bank Street",
                "21",
                "Main office",
                "America/Toronto"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                13,
                13400000,
                2500,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-fi-branch";
            input.displayName = "Horizon Professional Services Helsinki";
            input.country = "FI";
            input.locale = "fi-FI";
            input.currency = "EUR";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.personalIdentityCode = "050594U902L";
            input.address = new Address(
                "Helsinki",
                "Kallio",
                "Hämeentie",
                "21",
                "Main office",
                "Europe/Helsinki"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                14,
                13400000,
                2600,
                true,
                false
            );
            input.preferences = new Preferences(
                "fi",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "fi-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-de-branch";
            input.displayName = "Horizon Professional Services Berlin";
            input.country = "DE";
            input.locale = "de-DE";
            input.currency = "EUR";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.bsnr = "351234567";
            input.identifiers.drivingLicense = "HH98765432C";
            input.identifiers.handelsregisternummer = "HRA 12345";
            input.identifiers.healthInsuranceNumber = "C000500021";
            input.identifiers.identityCardNumber = "L01X00T44";
            input.identifiers.licensePlate = "HH-AB-1234";
            input.identifiers.lanr = "987654401";
            input.identifiers.passportNumber = "L01X00T44";
            input.identifiers.postalCode = "99998";
            input.identifiers.socialSecurityNumber = "65070803A019";
            input.identifiers.taxId = "98765432106";
            input.identifiers.taxNumber = "0281508150123";
            input.identifiers.vatId = "DE 136695976";
            input.address = new Address(
                "Berlin",
                "Mitte",
                "Friedrichstraße",
                "21",
                "Main office",
                "Europe/Berlin"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                15,
                13400000,
                2700,
                true,
                false
            );
            input.preferences = new Preferences(
                "de",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "de-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-in-branch";
            input.displayName = "Horizon Professional Services Bengaluru";
            input.country = "IN";
            input.locale = "en-IN";
            input.currency = "USD";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.aadhaarNumber = "3998 7654 3211";
            input.identifiers.gstin = "07PQRST6789K1Z2";
            input.identifiers.pan = "ABCPD1234Z";
            input.identifiers.passportNumber = "A3456781";
            input.identifiers.vehicleRegistration = "KA99ME3456";
            input.identifiers.voterId = "KSD1287349";
            input.address = new Address(
                "Bengaluru",
                "Indiranagar",
                "Market Road",
                "21",
                "Main office",
                "Asia/Kolkata"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                16,
                13400000,
                2800,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-it-branch";
            input.displayName = "Horizon Professional Services Milano";
            input.country = "IT";
            input.locale = "it-IT";
            input.currency = "EUR";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.driverLicense = "AA0123456B";
            input.identifiers.fiscalCode = "AAAAAA00B11C333N";
            input.identifiers.identityCardNumber = "AA12345aa";
            input.identifiers.passportNumber = "aa7654321";
            input.identifiers.vatCode = "01333550_323";
            input.address = new Address(
                "Milano",
                "Brera",
                "Via Solferino",
                "21",
                "Main office",
                "Europe/Rome"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                17,
                13400000,
                2900,
                true,
                false
            );
            input.preferences = new Preferences(
                "it",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "it-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-kr-branch";
            input.displayName = "Horizon Professional Services Seoul";
            input.country = "KR";
            input.locale = "ko-KR";
            input.currency = "KRW";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.brn = "104-86-56659";
            input.identifiers.driverLicense = "112212345612";
            input.identifiers.frn = "9111245678901";
            input.identifiers.passportNumber = "M12345678";
            input.identifiers.rrn = "9601211234567";
            input.address = new Address(
                "Seoul",
                "Mapo",
                "World Cup Road",
                "21",
                "Main office",
                "Asia/Seoul"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                18,
                13400000,
                3000,
                true,
                false
            );
            input.preferences = new Preferences(
                "ko",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "ko-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-ng-branch";
            input.displayName = "Horizon Professional Services Lagos";
            input.country = "NG";
            input.locale = "en-NG";
            input.currency = "USD";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nin = "12345678902";
            input.identifiers.vehicleRegistration = "KJA-999PZ";
            input.address = new Address(
                "Lagos",
                "Ikeja",
                "Allen Avenue",
                "21",
                "Main office",
                "Africa/Lagos"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                19,
                13400000,
                3100,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-ph-branch";
            input.displayName = "Horizon Professional Services Manila";
            input.country = "PH";
            input.locale = "en-PH";
            input.currency = "USD";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.passportNumber = "AA0000000";
            input.identifiers.tin = "000-123-456";
            input.identifiers.umid = "001112345678";
            input.address = new Address(
                "Manila",
                "Makati",
                "Ayala Avenue",
                "21",
                "Main office",
                "Asia/Manila"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                20,
                13400000,
                3200,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-pl-branch";
            input.displayName = "Horizon Professional Services Warszawa";
            input.country = "PL";
            input.locale = "pl-PL";
            input.currency = "EUR";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.pesel = "44051401458";
            input.address = new Address(
                "Warszawa",
                "Śródmieście",
                "Marszałkowska",
                "21",
                "Main office",
                "Europe/Warsaw"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                21,
                13400000,
                3300,
                true,
                false
            );
            input.preferences = new Preferences(
                "pl",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "pl-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-sg-branch";
            input.displayName = "Horizon Professional Services Singapore";
            input.country = "SG";
            input.locale = "en-SG";
            input.currency = "USD";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nricFin = "F2346401L";
            input.identifiers.uen = "T16RF0037C";
            input.address = new Address(
                "Singapore",
                "Outram",
                "Neil Road",
                "21",
                "Main office",
                "Asia/Singapore"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                22,
                13400000,
                3400,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-za-branch";
            input.displayName = "Horizon Professional Services Cape Town";
            input.country = "ZA";
            input.locale = "en-ZA";
            input.currency = "USD";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.companyRegistrationNumber = "CK1998/654321";
            input.identifiers.driverLicense = "4046048YPC9T";
            input.identifiers.idNumber = "0002294321191";
            input.identifiers.incomeTaxNumber = "1234567890";
            input.identifiers.licensePlate = "PMG017GP";
            input.identifiers.passportNumber = "T11223344";
            input.identifiers.trafficRegisterNumber = "6001015000076";
            input.identifiers.vatNumber = "4170229407";
            input.address = new Address(
                "Cape Town",
                "Gardens",
                "Kloof Street",
                "21",
                "Main office",
                "Africa/Johannesburg"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                23,
                13400000,
                3500,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-es-branch";
            input.displayName = "Horizon Professional Services Madrid";
            input.country = "ES";
            input.locale = "es-ES";
            input.currency = "EUR";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.nie = "Y8063915-Z";
            input.identifiers.nif = "55555555-K";
            input.identifiers.passportNumber = "xyz987654";
            input.address = new Address(
                "Madrid",
                "Centro",
                "Calle Mayor",
                "21",
                "Main office",
                "Europe/Madrid"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                24,
                13400000,
                3600,
                true,
                false
            );
            input.preferences = new Preferences(
                "es",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "es-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-se-branch";
            input.displayName = "Horizon Professional Services Stockholm";
            input.country = "SE";
            input.locale = "sv-SE";
            input.currency = "EUR";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.organisationsnummer = "2120000142";
            input.identifiers.personnummer = "200109022392";
            input.address = new Address(
                "Stockholm",
                "Södermalm",
                "Götgatan",
                "21",
                "Main office",
                "Europe/Stockholm"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                25,
                13400000,
                3700,
                true,
                false
            );
            input.preferences = new Preferences(
                "sv",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "sv-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-th-branch";
            input.displayName = "Horizon Professional Services Bangkok";
            input.country = "TH";
            input.locale = "th-TH";
            input.currency = "USD";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.tnin = "2345678901234";
            input.address = new Address(
                "Bangkok",
                "Watthana",
                "Sukhumvit Road",
                "21",
                "Main office",
                "Asia/Bangkok"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                26,
                13400000,
                3800,
                true,
                false
            );
            input.preferences = new Preferences(
                "th",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "th-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-tr-branch";
            input.displayName = "Horizon Professional Services Istanbul";
            input.country = "TR";
            input.locale = "tr-TR";
            input.currency = "EUR";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.licensePlate = "34 ABC 1234";
            input.identifiers.nationalIdNumber = "36493665440";
            input.address = new Address(
                "Istanbul",
                "Kadıköy",
                "Bahariye Street",
                "21",
                "Main office",
                "Europe/Istanbul"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                27,
                13400000,
                3900,
                true,
                false
            );
            input.preferences = new Preferences(
                "tr",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "tr-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-gb-branch";
            input.displayName = "Horizon Professional Services London";
            input.country = "GB";
            input.locale = "en-GB";
            input.currency = "GBP";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.drivingLicence = "SMITH862310AB1CD";
            input.identifiers.nhsNumber = "401-023-2137";
            input.identifiers.nino = "tw987654a";
            input.identifiers.passportNumber = "XY9876543";
            input.identifiers.postcode = "DN551PT";
            input.identifiers.vehicleRegistration = "ABC 1D";
            input.address = new Address(
                "London",
                "Camden",
                "High Street",
                "21",
                "Main office",
                "Europe/London"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                28,
                13400000,
                4000,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "immediate",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        {
            MerchantInput input = new MerchantInput();
            input.externalReference = "horizon-professionals-us-branch";
            input.displayName = "Horizon Professional Services Seattle";
            input.country = "US";
            input.locale = "en-US";
            input.currency = "USD";
            input.contactName = "Mina Santos";
            input.contactRole = "Regional operations manager";
            input.identifiers = new DocumentSet();
            input.identifiers.routingNumber = "3222-7162-7";
            input.identifiers.deaNumber = "BB1388568";
            input.identifiers.bankAccount = "945456787654";
            input.identifiers.driverLicense = "H12234567";
            input.identifiers.memberId = "AbC123456";
            input.identifiers.priorAuthorizationNumber = "987654321";
            input.identifiers.claimNumber = "CLM123456789012345";
            input.identifiers.prescriptionNumber = "RX123456";
            input.identifiers.referralNumber = "2025001234";
            input.identifiers.providerTaxId = "20-1234567";
            input.identifiers.itinNumber = "911-70-1234";
            input.identifiers.mbiNumber = "3CD5-FG7-HJ89";
            input.identifiers.npiNumber = "1234 567 893";
            input.identifiers.passportNumber = "912803456";
            input.identifiers.ssn = "987-65-4326";
            input.address = new Address(
                "Seattle",
                "Fremont",
                "Fremont Avenue",
                "21",
                "Main office",
                "America/Los_Angeles"
            );
            input.business = new Business(
                "services",
                "company",
                2014,
                29,
                13400000,
                4100,
                true,
                false
            );
            input.preferences = new Preferences(
                "en",
                "portal",
                "digest",
                false,
                true
            );
            input.limits = new Limits(
                50000,
                250000,
                30,
                2
            );
            input.capabilities = new Capabilities(
                true,
                true,
                true,
                false
            );
            input.tags = List.of("services", "partner", "priority");
            input.statementDescriptor = "horizon-professional";
            input.supportQueue = "en-merchant-support";
            input.onboardingChannel = "partner";
            merchants.add(input);
        }
        return new Scenario(tenant, verification,
            18900, 2400, 5000, 18, 81, List.copyOf(merchants));
    }
    static List<Scenario> buildScenarios() {
        return List.of(
            buildAtlasMarketScenario(),
            buildHarborServicesScenario(),
            buildCedarClinicsScenario(),
            buildNorthstarSupplyScenario(),
            buildOrchardStoresScenario(),
            buildRiverWorkshopsScenario(),
            buildLighthouseCareScenario(),
            buildSummitMakersScenario(),
            buildMeadowCommerceScenario(),
            buildHorizonProfessionalsScenario()
        );
    }
}
