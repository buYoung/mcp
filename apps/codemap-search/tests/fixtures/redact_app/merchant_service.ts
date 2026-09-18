// Executable, offline service fixture for source-redaction integration tests.
// Sample documents come from the Presidio examples attributed in vendor/presidio/LICENSE.

export type CountryCode =
  | "AU" | "CA" | "FI" | "DE" | "IN" | "IT" | "KR" | "NG" | "PH"
  | "PL" | "SG" | "ZA" | "ES" | "SE" | "TH" | "TR" | "GB" | "US";

export type Currency = "USD" | "EUR" | "GBP" | "AUD" | "CAD" | "KRW" | "JPY";
export type MerchantStatus = "draft" | "submitted" | "approved" | "suspended" | "closed";
export type PaymentStatus = "authorized" | "captured" | "partially_refunded" | "refunded" | "voided";
export type ReviewDecision = "approve" | "request_changes" | "reject";
export type Role = "owner" | "reviewer" | "billing" | "support";

export interface Clock {
  nowMs(): number;
}

export interface IdFactory {
  next(kind: string): string;
}

export interface Principal {
  tenantId: string;
  actorId: string;
  roles: ReadonlyArray<Role>;
}

export interface RuntimeConfig {
  timeoutMs: number;
  batchSize: number;
  maxPageSize: number;
  maxRetryAttempts: number;
  retryDelayMs: number;
  maxPayloadBytes: number;
  checksumAlgorithm: "sha256";
  allowedCurrencies: ReadonlyArray<Currency>;
  providerToken: string;
}

export interface TenantInput {
  slug: string;
  displayName: string;
  defaultCurrency: Currency;
  defaultLocale: string;
  enabledCountries: ReadonlyArray<CountryCode>;
  plan: "standard" | "enterprise";
  monthlyDocumentLimit: number;
  isExportEnabled: boolean;
}

export interface Tenant extends TenantInput {
  id: string;
  isActive: boolean;
  revision: number;
  createdAtMs: number;
}

export interface ContactPreferences {
  language: string;
  invoiceDelivery: "portal" | "email";
  notificationMode: "digest" | "immediate";
  hasMarketingConsent: boolean;
  hasServiceConsent: boolean;
}

export interface TradingAddress {
  city: string;
  district: string;
  streetName: string;
  buildingNumber: string;
  unitName: string;
  timeZone: string;
}

export interface BusinessProfile {
  sector: "retail" | "services" | "healthcare" | "manufacturing";
  legalForm: "company" | "sole_trader" | "partnership";
  foundedYear: number;
  employeeCount: number;
  annualTurnoverCents: number;
  averageOrderCents: number;
  hasPhysicalStore: boolean;
  hasOnlineStore: boolean;
}

export interface MerchantLimits {
  singlePaymentCents: number;
  dailyPaymentCents: number;
  refundWindowDays: number;
  settlementDelayDays: number;
}

export interface MerchantCapabilities {
  canAcceptPayments: boolean;
  canIssueRefunds: boolean;
  canRequestPayouts: boolean;
  shouldRequireSecondReviewer: boolean;
}

export interface MerchantInput {
  externalReference: string;
  displayName: string;
  country: CountryCode;
  locale: string;
  currency: Currency;
  contactName: string;
  contactRole: string;
  identifiers: Readonly<Record<string, string>>;
  address: TradingAddress;
  business: BusinessProfile;
  preferences: ContactPreferences;
  limits: MerchantLimits;
  capabilities: MerchantCapabilities;
  tags: ReadonlyArray<string>;
  statementDescriptor: string;
  supportQueue: string;
  onboardingChannel: "partner" | "self_service";
}

export interface Merchant extends MerchantInput {
  id: string;
  tenantId: string;
  status: MerchantStatus;
  revision: number;
  createdAtMs: number;
  updatedAtMs: number;
  submittedAtMs: number | null;
  approvedAtMs: number | null;
  reviewNotes: ReadonlyArray<string>;
}

export interface PaymentInput {
  merchantId: string;
  currency: Currency;
  amountCents: number;
  orderReference: string;
  description: string;
  idempotencyKey: string;
}

export interface Payment extends PaymentInput {
  id: string;
  tenantId: string;
  status: PaymentStatus;
  capturedCents: number;
  refundedCents: number;
  revision: number;
  createdAtMs: number;
  capturedAtMs: number | null;
}

export interface Refund {
  id: string;
  tenantId: string;
  paymentId: string;
  amountCents: number;
  reason: string;
  idempotencyKey: string;
  createdAtMs: number;
}

export interface Payout {
  id: string;
  tenantId: string;
  merchantId: string;
  amountCents: number;
  currency: Currency;
  bankAccountReference: string;
  status: "scheduled" | "completed" | "cancelled";
  availableAtMs: number;
  revision: number;
}

export interface LedgerEntry {
  id: string;
  tenantId: string;
  merchantId: string;
  currency: Currency;
  amountCents: number;
  category: "capture" | "refund" | "payout" | "payout_cancelled";
  referenceId: string;
  createdAtMs: number;
}

export interface AuditEvent {
  sequence: number;
  tenantId: string;
  actorId: string;
  action: string;
  resourceId: string;
  details: Readonly<Record<string, string | number | boolean>>;
  createdAtMs: number;
}

export interface OutboxMessage {
  id: string;
  tenantId: string;
  topic: string;
  resourceId: string;
  attempts: number;
  availableAtMs: number;
  isDelivered: boolean;
}

export interface SupportTicket {
  id: string;
  tenantId: string;
  merchantId: string;
  subject: string;
  queue: string;
  status: "open" | "assigned" | "resolved";
  assigneeId: string | null;
  messages: ReadonlyArray<{ actorId: string; body: string; createdAtMs: number }>;
  revision: number;
}

export interface Page<T> {
  items: ReadonlyArray<T>;
  nextCursor: string | null;
  total: number;
}

export interface VerificationProfile {
  cardNumber: string;
  bitcoinAddress: string;
  openedAt: string;
  email: string;
  iban: string;
  ipAddress: string;
  macAddress: string;
  website: string;
  correlationId: string;
}

export interface TenantScenario {
  tenant: TenantInput;
  verification: VerificationProfile;
  merchants: ReadonlyArray<MerchantInput>;
  paymentAmountCents: number;
  partialRefundCents: number;
  payoutAmountCents: number;
  expectedMerchantCount: number;
  expectedDocumentCount: number;
}

export class DomainError extends Error {
  readonly code: string;
  readonly status: number;

  constructor(code: string, message: string, status = 400) {
    super(message);
    this.name = "DomainError";
    this.code = code;
    this.status = status;
  }
}

function ensure(condition: unknown, code: string, message: string, status = 400): asserts condition {
  if (!condition) {
    throw new DomainError(code, message, status);
  }
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function requireText(value: unknown, field: string, maximum = 200): string {
  ensure(typeof value === "string", "invalid_field", `${field} must be a string`);
  const normalized = value.trim();
  ensure(normalized.length > 0, "missing_field", `${field} is required`);
  ensure(normalized.length <= maximum, "invalid_field", `${field} is too long`);
  return normalized;
}

function requireAmount(amountCents: number, field = "amountCents"): number {
  ensure(Number.isSafeInteger(amountCents), "invalid_amount", `${field} must be an integer`);
  ensure(amountCents > 0, "invalid_amount", `${field} must be positive`);
  return amountCents;
}

function requireRole(principal: Principal, role: Role): void {
  ensure(principal.roles.includes(role) || principal.roles.includes("owner"), "forbidden", "Role is not allowed", 403);
}

function requireTenant(principal: Principal, tenantId: string): void {
  ensure(principal.tenantId === tenantId, "not_found", "Resource was not found", 404);
}

function verifyRevision(actual: number, expected: number): void {
  ensure(Number.isSafeInteger(expected), "invalid_revision", "A revision is required");
  ensure(actual === expected, "conflict", "Resource changed; reload it before retrying", 409);
}

function normalizeTags(tags: ReadonlyArray<string>): string[] {
  const values = tags.map(tag => requireText(tag, "tag", 40).toLowerCase());
  return [...new Set(values)].sort();
}

function pageItems<T extends { id: string }>(items: ReadonlyArray<T>, cursor: string | null, limit: number): Page<T> {
  ensure(Number.isInteger(limit) && limit > 0 && limit <= 100, "invalid_page", "Invalid page size");
  const sorted = [...items].sort((left, right) => left.id.localeCompare(right.id));
  const start = cursor === null ? 0 : sorted.findIndex(item => item.id === cursor) + 1;
  ensure(cursor === null || start > 0, "invalid_cursor", "Cursor no longer exists");
  const selected = sorted.slice(start, start + limit);
  const hasMore = start + selected.length < sorted.length;
  return {
    items: selected.map(clone),
    nextCursor: hasMore ? selected[selected.length - 1]!.id : null,
    total: sorted.length,
  };
}

export function loadRuntimeConfig(values: Readonly<Record<string, string | undefined>>): RuntimeConfig {
  const readPositiveInteger = (name: string, fallback: number): number => {
    const raw = values[name];
    const value = raw === undefined ? fallback : Number(raw);
    ensure(Number.isSafeInteger(value) && value > 0, "invalid_config", `${name} must be positive`);
    return value;
  };
  const providerToken = values.PROVIDER_TOKEN ?? "";
  return {
    timeoutMs: readPositiveInteger("REQUEST_TIMEOUT_MS", 10000),
    batchSize: readPositiveInteger("BATCH_SIZE", 5000),
    maxPageSize: readPositiveInteger("MAX_PAGE_SIZE", 100),
    maxRetryAttempts: readPositiveInteger("MAX_RETRY_ATTEMPTS", 3),
    retryDelayMs: readPositiveInteger("RETRY_DELAY_MS", 250),
    maxPayloadBytes: readPositiveInteger("MAX_PAYLOAD_BYTES", 65536),
    checksumAlgorithm: "sha256",
    allowedCurrencies: ["USD", "EUR", "GBP", "AUD", "CAD", "KRW", "JPY"],
    providerToken,
  };
}

export class ManualClock implements Clock {
  private valueMs: number;

  constructor(valueMs = Date.UTC(2026, 8, 1)) {
    this.valueMs = valueMs;
  }

  nowMs(): number {
    return this.valueMs;
  }

  advanceMs(deltaMs: number): void {
    ensure(deltaMs >= 0, "invalid_clock", "Clock cannot move backwards");
    this.valueMs += deltaMs;
  }
}

export class SequentialIds implements IdFactory {
  private readonly counters = new Map<string, number>();

  next(kind: string): string {
    const value = (this.counters.get(kind) ?? 0) + 1;
    this.counters.set(kind, value);
    return `${kind}_${String(value).padStart(6, "0")}`;
  }
}

export class Table<T extends { id: string }> {
  private rows = new Map<string, T>();

  insert(row: T): T {
    ensure(!this.rows.has(row.id), "conflict", "Resource already exists", 409);
    this.rows.set(row.id, clone(row));
    return clone(row);
  }

  get(id: string): T {
    const row = this.rows.get(id);
    ensure(row !== undefined, "not_found", "Resource was not found", 404);
    return clone(row);
  }

  replace(row: T): T {
    ensure(this.rows.has(row.id), "not_found", "Resource was not found", 404);
    this.rows.set(row.id, clone(row));
    return clone(row);
  }

  find(predicate: (row: T) => boolean): T | undefined {
    for (const row of this.rows.values()) {
      if (predicate(row)) {
        return clone(row);
      }
    }
    return undefined;
  }

  list(predicate: (row: T) => boolean = () => true): T[] {
    return [...this.rows.values()].filter(predicate).map(clone);
  }

  snapshot(): ReadonlyArray<T> {
    return this.list();
  }

  restore(rows: ReadonlyArray<T>): void {
    this.rows = new Map(rows.map(row => [row.id, clone(row)]));
  }
}

export class Database {
  readonly tenants = new Table<Tenant>();
  readonly merchants = new Table<Merchant>();
  readonly payments = new Table<Payment>();
  readonly refunds = new Table<Refund>();
  readonly payouts = new Table<Payout>();
  readonly ledger = new Table<LedgerEntry>();
  readonly tickets = new Table<SupportTicket>();
  readonly outbox = new Table<OutboxMessage>();
  readonly audit: AuditEvent[] = [];

  transaction<T>(operation: () => T): T {
    const snapshots = {
      tenants: this.tenants.snapshot(),
      merchants: this.merchants.snapshot(),
      payments: this.payments.snapshot(),
      refunds: this.refunds.snapshot(),
      payouts: this.payouts.snapshot(),
      ledger: this.ledger.snapshot(),
      tickets: this.tickets.snapshot(),
      outbox: this.outbox.snapshot(),
      auditLength: this.audit.length,
    };
    try {
      return operation();
    } catch (error) {
      this.tenants.restore(snapshots.tenants);
      this.merchants.restore(snapshots.merchants);
      this.payments.restore(snapshots.payments);
      this.refunds.restore(snapshots.refunds);
      this.payouts.restore(snapshots.payouts);
      this.ledger.restore(snapshots.ledger);
      this.tickets.restore(snapshots.tickets);
      this.outbox.restore(snapshots.outbox);
      this.audit.splice(snapshots.auditLength);
      throw error;
    }
  }
}

export class EventJournal {
  constructor(
    private readonly database: Database,
    private readonly clock: Clock,
    private readonly ids: IdFactory,
  ) {}

  record(principal: Principal, action: string, resourceId: string, details: AuditEvent["details"] = {}): void {
    this.database.audit.push({
      sequence: this.database.audit.length + 1,
      tenantId: principal.tenantId,
      actorId: principal.actorId,
      action,
      resourceId,
      details: clone(details),
      createdAtMs: this.clock.nowMs(),
    });
    this.database.outbox.insert({
      id: this.ids.next("event"),
      tenantId: principal.tenantId,
      topic: action,
      resourceId,
      attempts: 0,
      availableAtMs: this.clock.nowMs(),
      isDelivered: false,
    });
  }

  list(principal: Principal, resourceId: string): ReadonlyArray<AuditEvent> {
    requireRole(principal, "support");
    return this.database.audit
      .filter(event => event.tenantId === principal.tenantId && event.resourceId === resourceId)
      .map(clone);
  }
}

export class TenantService {
  constructor(
    private readonly database: Database,
    private readonly clock: Clock,
    private readonly ids: IdFactory,
  ) {}

  create(input: TenantInput): Tenant {
    const slug = requireText(input.slug, "slug", 50).toLowerCase();
    ensure(/^[a-z][a-z0-9-]+$/.test(slug), "invalid_slug", "Tenant slug has unsupported characters");
    ensure(!this.database.tenants.find(tenant => tenant.slug === slug), "conflict", "Tenant slug is taken", 409);
    ensure(input.enabledCountries.length > 0, "invalid_country", "At least one country must be enabled");
    return this.database.tenants.insert({
      ...clone(input),
      id: this.ids.next("tenant"),
      slug,
      displayName: requireText(input.displayName, "displayName"),
      enabledCountries: [...new Set(input.enabledCountries)],
      isActive: true,
      revision: 1,
      createdAtMs: this.clock.nowMs(),
    });
  }

  requireActive(tenantId: string): Tenant {
    const tenant = this.database.tenants.get(tenantId);
    ensure(tenant.isActive, "tenant_disabled", "Tenant is disabled", 403);
    return tenant;
  }

  setActive(principal: Principal, isActive: boolean, revision: number): Tenant {
    requireRole(principal, "owner");
    const tenant = this.database.tenants.get(principal.tenantId);
    verifyRevision(tenant.revision, revision);
    return this.database.tenants.replace({ ...tenant, isActive, revision: tenant.revision + 1 });
  }
}

export interface JurisdictionPolicy {
  country: CountryCode;
  displayName: string;
  supportedLanguages: ReadonlyArray<string>;
  supportedDocumentFields: ReadonlyArray<string>;
  reviewQueue: string;
}

export function findJurisdiction(country: CountryCode): JurisdictionPolicy {
  const policy = JURISDICTIONS.find(entry => entry.country === country);
  ensure(policy !== undefined, "unsupported_country", "Country is not supported");
  return policy;
}

function validateIdentifiers(country: CountryCode, identifiers: Readonly<Record<string, string>>): Record<string, string> {
  const policy = findJurisdiction(country);
  const entries = Object.entries(identifiers);
  ensure(entries.length > 0, "missing_documents", "Verification documents are required");
  const result: Record<string, string> = {};
  for (const [field, value] of entries) {
    ensure(policy.supportedDocumentFields.includes(field), "unknown_document", "Document type is not supported in this country");
    result[field] = requireText(value, field, 160);
  }
  return result;
}

function validateMerchantInput(input: MerchantInput, tenant: Tenant): MerchantInput {
  ensure(tenant.enabledCountries.includes(input.country), "country_disabled", "Country is disabled for this tenant");
  const policy = findJurisdiction(input.country);
  ensure(policy.supportedLanguages.includes(input.preferences.language), "invalid_language", "Unsupported statement language");
  ensure(input.preferences.hasServiceConsent, "consent_required", "Service consent is required");
  requireAmount(input.limits.singlePaymentCents, "singlePaymentCents");
  requireAmount(input.limits.dailyPaymentCents, "dailyPaymentCents");
  ensure(input.limits.dailyPaymentCents >= input.limits.singlePaymentCents, "invalid_limits", "Daily limit must cover a single payment");
  ensure(input.limits.refundWindowDays > 0, "invalid_limits", "Refund window must be positive");
  ensure(input.limits.settlementDelayDays >= 0, "invalid_limits", "Settlement delay cannot be negative");
  ensure(Number.isInteger(input.business.employeeCount) && input.business.employeeCount >= 0, "invalid_business", "Invalid employee count");
  ensure(input.business.foundedYear >= 1900, "invalid_business", "Invalid founding year");
  return {
    ...clone(input),
    displayName: requireText(input.displayName, "displayName"),
    externalReference: requireText(input.externalReference, "externalReference", 80),
    contactName: requireText(input.contactName, "contactName"),
    contactRole: requireText(input.contactRole, "contactRole", 80),
    identifiers: validateIdentifiers(input.country, input.identifiers),
    statementDescriptor: requireText(input.statementDescriptor, "statementDescriptor", 30),
    tags: normalizeTags(input.tags),
  };
}

export class MerchantService {
  constructor(
    private readonly database: Database,
    private readonly tenants: TenantService,
    private readonly journal: EventJournal,
    private readonly clock: Clock,
    private readonly ids: IdFactory,
  ) {}

  register(principal: Principal, input: MerchantInput): Merchant {
    requireRole(principal, "owner");
    const tenant = this.tenants.requireActive(principal.tenantId);
    const normalized = validateMerchantInput(input, tenant);
    const existing = this.database.merchants.find(row =>
      row.tenantId === tenant.id && row.externalReference === normalized.externalReference,
    );
    ensure(existing === undefined, "duplicate_reference", "External reference already exists", 409);
    return this.database.transaction(() => {
      const merchant = this.database.merchants.insert({
        ...normalized,
        id: this.ids.next("merchant"),
        tenantId: tenant.id,
        status: "draft",
        revision: 1,
        createdAtMs: this.clock.nowMs(),
        updatedAtMs: this.clock.nowMs(),
        submittedAtMs: null,
        approvedAtMs: null,
        reviewNotes: [],
      });
      this.journal.record(principal, "merchant.registered", merchant.id, { country: merchant.country });
      return merchant;
    });
  }

  get(principal: Principal, merchantId: string): Merchant {
    this.tenants.requireActive(principal.tenantId);
    const merchant = this.database.merchants.get(merchantId);
    requireTenant(principal, merchant.tenantId);
    return merchant;
  }

  list(principal: Principal, cursor: string | null = null, limit = 20): Page<Merchant> {
    this.tenants.requireActive(principal.tenantId);
    const merchants = this.database.merchants.list(row => row.tenantId === principal.tenantId);
    return pageItems(merchants, cursor, limit);
  }

  updateContact(principal: Principal, merchantId: string, input: Pick<MerchantInput, "contactName" | "contactRole" | "preferences">, revision: number): Merchant {
    requireRole(principal, "owner");
    const merchant = this.get(principal, merchantId);
    verifyRevision(merchant.revision, revision);
    ensure(merchant.status !== "closed", "invalid_state", "Closed merchant cannot be changed", 409);
    ensure(input.preferences.hasServiceConsent, "consent_required", "Service consent is required");
    const updated = this.database.merchants.replace({
      ...merchant,
      contactName: requireText(input.contactName, "contactName"),
      contactRole: requireText(input.contactRole, "contactRole"),
      preferences: clone(input.preferences),
      revision: merchant.revision + 1,
      updatedAtMs: this.clock.nowMs(),
    });
    this.journal.record(principal, "merchant.contact_updated", merchant.id);
    return updated;
  }

  submit(principal: Principal, merchantId: string, revision: number): Merchant {
    requireRole(principal, "owner");
    const merchant = this.get(principal, merchantId);
    verifyRevision(merchant.revision, revision);
    ensure(merchant.status === "draft", "invalid_state", "Only a draft can be submitted", 409);
    ensure(Object.keys(merchant.identifiers).length > 0, "missing_documents", "No documents attached");
    const submitted = this.database.merchants.replace({
      ...merchant,
      status: "submitted",
      submittedAtMs: this.clock.nowMs(),
      updatedAtMs: this.clock.nowMs(),
      revision: merchant.revision + 1,
    });
    this.journal.record(principal, "merchant.submitted", merchant.id, {
      queue: findJurisdiction(merchant.country).reviewQueue,
      documentCount: Object.keys(merchant.identifiers).length,
    });
    return submitted;
  }

  review(principal: Principal, merchantId: string, decision: ReviewDecision, note: string, revision: number): Merchant {
    requireRole(principal, "reviewer");
    const merchant = this.get(principal, merchantId);
    verifyRevision(merchant.revision, revision);
    ensure(merchant.status === "submitted", "invalid_state", "Only a submitted merchant can be reviewed", 409);
    const normalizedNote = requireText(note, "reviewNote", 400);
    const status = decision === "approve" ? "approved" : decision === "reject" ? "closed" : "draft";
    const reviewed = this.database.merchants.replace({
      ...merchant,
      status,
      approvedAtMs: decision === "approve" ? this.clock.nowMs() : null,
      reviewNotes: [...merchant.reviewNotes, normalizedNote],
      revision: merchant.revision + 1,
      updatedAtMs: this.clock.nowMs(),
    });
    this.journal.record(principal, "merchant.reviewed", merchant.id, { decision });
    return reviewed;
  }

  suspend(principal: Principal, merchantId: string, reason: string, revision: number): Merchant {
    requireRole(principal, "reviewer");
    const merchant = this.get(principal, merchantId);
    verifyRevision(merchant.revision, revision);
    ensure(merchant.status === "approved", "invalid_state", "Only an approved merchant can be suspended", 409);
    const suspended = this.database.merchants.replace({
      ...merchant,
      status: "suspended",
      reviewNotes: [...merchant.reviewNotes, requireText(reason, "reason", 400)],
      revision: merchant.revision + 1,
      updatedAtMs: this.clock.nowMs(),
    });
    this.journal.record(principal, "merchant.suspended", merchant.id);
    return suspended;
  }

  resume(principal: Principal, merchantId: string, revision: number): Merchant {
    requireRole(principal, "reviewer");
    const merchant = this.get(principal, merchantId);
    verifyRevision(merchant.revision, revision);
    ensure(merchant.status === "suspended", "invalid_state", "Only a suspended merchant can be resumed", 409);
    const resumed = this.database.merchants.replace({
      ...merchant,
      status: "approved",
      revision: merchant.revision + 1,
      updatedAtMs: this.clock.nowMs(),
    });
    this.journal.record(principal, "merchant.resumed", merchant.id);
    return resumed;
  }
}

export class LedgerService {
  constructor(
    private readonly database: Database,
    private readonly clock: Clock,
    private readonly ids: IdFactory,
  ) {}

  post(merchant: Merchant, amountCents: number, category: LedgerEntry["category"], referenceId: string): LedgerEntry {
    ensure(Number.isSafeInteger(amountCents) && amountCents !== 0, "invalid_amount", "Ledger entry must be a nonzero integer");
    return this.database.ledger.insert({
      id: this.ids.next("ledger"),
      tenantId: merchant.tenantId,
      merchantId: merchant.id,
      currency: merchant.currency,
      amountCents,
      category,
      referenceId,
      createdAtMs: this.clock.nowMs(),
    });
  }

  balance(principal: Principal, merchantId: string, currency: Currency): number {
    const entries = this.database.ledger.list(row =>
      row.tenantId === principal.tenantId && row.merchantId === merchantId && row.currency === currency,
    );
    return entries.reduce((balance, entry) => balance + entry.amountCents, 0);
  }

  reconcile(principal: Principal, merchantId: string): Readonly<Record<Currency, number>> {
    const totals: Record<Currency, number> = { USD: 0, EUR: 0, GBP: 0, AUD: 0, CAD: 0, KRW: 0, JPY: 0 };
    for (const entry of this.database.ledger.list(row => row.tenantId === principal.tenantId && row.merchantId === merchantId)) {
      totals[entry.currency] += entry.amountCents;
    }
    return totals;
  }
}

export class PaymentService {
  constructor(
    private readonly database: Database,
    private readonly merchants: MerchantService,
    private readonly ledger: LedgerService,
    private readonly journal: EventJournal,
    private readonly clock: Clock,
    private readonly ids: IdFactory,
  ) {}

  authorize(principal: Principal, input: PaymentInput): Payment {
    requireRole(principal, "billing");
    requireAmount(input.amountCents);
    const merchant = this.merchants.get(principal, input.merchantId);
    ensure(merchant.status === "approved", "merchant_unavailable", "Merchant is not approved", 409);
    ensure(merchant.capabilities.canAcceptPayments, "payments_disabled", "Payments are disabled", 403);
    ensure(merchant.currency === input.currency, "currency_mismatch", "Payment currency does not match merchant");
    const key = requireText(input.idempotencyKey, "idempotencyKey", 120);
    const previous = this.database.payments.find(row => row.tenantId === principal.tenantId && row.idempotencyKey === key);
    if (previous !== undefined) {
      ensure(previous.merchantId === input.merchantId && previous.amountCents === input.amountCents && previous.currency === input.currency, "idempotency_conflict", "Key was used for a different payment", 409);
      return previous;
    }
    ensure(input.amountCents <= merchant.limits.singlePaymentCents, "payment_limit", "Single payment limit exceeded");
    const dayStartMs = Math.floor(this.clock.nowMs() / 86400000) * 86400000;
    const committedCents = this.database.payments
      .list(row => row.merchantId === merchant.id && row.createdAtMs >= dayStartMs && row.status !== "voided")
      .reduce((total, row) => total + row.amountCents, 0);
    ensure(committedCents + input.amountCents <= merchant.limits.dailyPaymentCents, "daily_limit", "Daily payment limit exceeded");
    const payment = this.database.payments.insert({
      ...clone(input),
      id: this.ids.next("payment"),
      tenantId: principal.tenantId,
      idempotencyKey: key,
      orderReference: requireText(input.orderReference, "orderReference", 100),
      description: requireText(input.description, "description", 200),
      status: "authorized",
      capturedCents: 0,
      refundedCents: 0,
      revision: 1,
      createdAtMs: this.clock.nowMs(),
      capturedAtMs: null,
    });
    this.journal.record(principal, "payment.authorized", payment.id, { amountCents: payment.amountCents });
    return payment;
  }

  get(principal: Principal, paymentId: string): Payment {
    const payment = this.database.payments.get(paymentId);
    requireTenant(principal, payment.tenantId);
    return payment;
  }

  capture(principal: Principal, paymentId: string, revision: number): Payment {
    requireRole(principal, "billing");
    const payment = this.get(principal, paymentId);
    verifyRevision(payment.revision, revision);
    ensure(payment.status === "authorized", "invalid_state", "Payment cannot be captured", 409);
    const merchant = this.merchants.get(principal, payment.merchantId);
    ensure(merchant.status === "approved", "merchant_unavailable", "Merchant is not approved", 409);
    return this.database.transaction(() => {
      const captured = this.database.payments.replace({
        ...payment,
        status: "captured",
        capturedCents: payment.amountCents,
        capturedAtMs: this.clock.nowMs(),
        revision: payment.revision + 1,
      });
      this.ledger.post(merchant, payment.amountCents, "capture", payment.id);
      this.journal.record(principal, "payment.captured", payment.id, { amountCents: payment.amountCents });
      return captured;
    });
  }

  void(principal: Principal, paymentId: string, revision: number): Payment {
    requireRole(principal, "billing");
    const payment = this.get(principal, paymentId);
    verifyRevision(payment.revision, revision);
    ensure(payment.status === "authorized", "invalid_state", "Only an authorization can be voided", 409);
    const voided = this.database.payments.replace({ ...payment, status: "voided", revision: payment.revision + 1 });
    this.journal.record(principal, "payment.voided", payment.id);
    return voided;
  }

  refund(principal: Principal, paymentId: string, amountCents: number, reason: string, idempotencyKey: string): Refund {
    requireRole(principal, "billing");
    requireAmount(amountCents);
    const payment = this.get(principal, paymentId);
    const merchant = this.merchants.get(principal, payment.merchantId);
    ensure(merchant.capabilities.canIssueRefunds, "refunds_disabled", "Refunds are disabled", 403);
    const previous = this.database.refunds.find(row => row.tenantId === principal.tenantId && row.idempotencyKey === idempotencyKey);
    if (previous !== undefined) {
      ensure(previous.paymentId === paymentId && previous.amountCents === amountCents, "idempotency_conflict", "Key was used for another refund", 409);
      return previous;
    }
    ensure(payment.status === "captured" || payment.status === "partially_refunded", "invalid_state", "Payment cannot be refunded", 409);
    ensure(payment.refundedCents + amountCents <= payment.capturedCents, "refund_limit", "Refund exceeds captured amount");
    ensure(payment.capturedAtMs !== null, "invalid_state", "Capture timestamp is missing");
    const ageMs = this.clock.nowMs() - payment.capturedAtMs;
    ensure(ageMs <= merchant.limits.refundWindowDays * 86400000, "refund_expired", "Refund window has expired");
    return this.database.transaction(() => {
      const refundedCents = payment.refundedCents + amountCents;
      const refund = this.database.refunds.insert({
        id: this.ids.next("refund"),
        tenantId: principal.tenantId,
        paymentId,
        amountCents,
        reason: requireText(reason, "reason", 200),
        idempotencyKey: requireText(idempotencyKey, "idempotencyKey", 120),
        createdAtMs: this.clock.nowMs(),
      });
      this.database.payments.replace({
        ...payment,
        refundedCents,
        status: refundedCents === payment.capturedCents ? "refunded" : "partially_refunded",
        revision: payment.revision + 1,
      });
      this.ledger.post(merchant, -amountCents, "refund", refund.id);
      this.journal.record(principal, "payment.refunded", payment.id, { amountCents });
      return refund;
    });
  }
}

export class PayoutService {
  constructor(
    private readonly database: Database,
    private readonly merchants: MerchantService,
    private readonly ledger: LedgerService,
    private readonly journal: EventJournal,
    private readonly clock: Clock,
    private readonly ids: IdFactory,
  ) {}

  schedule(principal: Principal, merchantId: string, amountCents: number, bankAccountReference: string): Payout {
    requireRole(principal, "billing");
    requireAmount(amountCents);
    const merchant = this.merchants.get(principal, merchantId);
    ensure(merchant.status === "approved", "merchant_unavailable", "Merchant is not approved", 409);
    ensure(merchant.capabilities.canRequestPayouts, "payouts_disabled", "Payouts are disabled", 403);
    const balanceCents = this.ledger.balance(principal, merchant.id, merchant.currency);
    ensure(amountCents <= balanceCents, "insufficient_balance", "Payout exceeds available balance");
    return this.database.transaction(() => {
      const payout = this.database.payouts.insert({
        id: this.ids.next("payout"),
        tenantId: principal.tenantId,
        merchantId,
        amountCents,
        currency: merchant.currency,
        bankAccountReference: requireText(bankAccountReference, "bankAccountReference"),
        status: "scheduled",
        availableAtMs: this.clock.nowMs() + merchant.limits.settlementDelayDays * 86400000,
        revision: 1,
      });
      this.ledger.post(merchant, -amountCents, "payout", payout.id);
      this.journal.record(principal, "payout.scheduled", payout.id, { amountCents });
      return payout;
    });
  }

  complete(principal: Principal, payoutId: string, revision: number): Payout {
    requireRole(principal, "billing");
    const payout = this.database.payouts.get(payoutId);
    requireTenant(principal, payout.tenantId);
    verifyRevision(payout.revision, revision);
    ensure(payout.status === "scheduled", "invalid_state", "Payout is not scheduled", 409);
    ensure(payout.availableAtMs <= this.clock.nowMs(), "settlement_pending", "Settlement delay has not elapsed", 409);
    const completed = this.database.payouts.replace({ ...payout, status: "completed", revision: payout.revision + 1 });
    this.journal.record(principal, "payout.completed", payout.id);
    return completed;
  }

  cancel(principal: Principal, payoutId: string, revision: number): Payout {
    requireRole(principal, "billing");
    const payout = this.database.payouts.get(payoutId);
    requireTenant(principal, payout.tenantId);
    verifyRevision(payout.revision, revision);
    ensure(payout.status === "scheduled", "invalid_state", "Payout cannot be cancelled", 409);
    const merchant = this.merchants.get(principal, payout.merchantId);
    return this.database.transaction(() => {
      const cancelled = this.database.payouts.replace({ ...payout, status: "cancelled", revision: payout.revision + 1 });
      this.ledger.post(merchant, payout.amountCents, "payout_cancelled", payout.id);
      this.journal.record(principal, "payout.cancelled", payout.id);
      return cancelled;
    });
  }
}

export class SupportService {
  constructor(
    private readonly database: Database,
    private readonly merchants: MerchantService,
    private readonly journal: EventJournal,
    private readonly clock: Clock,
    private readonly ids: IdFactory,
  ) {}

  open(principal: Principal, merchantId: string, subject: string, body: string): SupportTicket {
    requireRole(principal, "support");
    const merchant = this.merchants.get(principal, merchantId);
    const ticket = this.database.tickets.insert({
      id: this.ids.next("ticket"),
      tenantId: principal.tenantId,
      merchantId,
      subject: requireText(subject, "subject", 120),
      queue: merchant.supportQueue,
      status: "open",
      assigneeId: null,
      messages: [{ actorId: principal.actorId, body: requireText(body, "body", 1000), createdAtMs: this.clock.nowMs() }],
      revision: 1,
    });
    this.journal.record(principal, "support.opened", ticket.id);
    return ticket;
  }

  assign(principal: Principal, ticketId: string, assigneeId: string, revision: number): SupportTicket {
    requireRole(principal, "support");
    const ticket = this.database.tickets.get(ticketId);
    requireTenant(principal, ticket.tenantId);
    verifyRevision(ticket.revision, revision);
    ensure(ticket.status !== "resolved", "invalid_state", "Resolved ticket cannot be assigned", 409);
    return this.database.tickets.replace({
      ...ticket,
      status: "assigned",
      assigneeId: requireText(assigneeId, "assigneeId", 80),
      revision: ticket.revision + 1,
    });
  }

  reply(principal: Principal, ticketId: string, body: string, revision: number): SupportTicket {
    requireRole(principal, "support");
    const ticket = this.database.tickets.get(ticketId);
    requireTenant(principal, ticket.tenantId);
    verifyRevision(ticket.revision, revision);
    ensure(ticket.status !== "resolved", "invalid_state", "Resolved ticket cannot receive replies", 409);
    return this.database.tickets.replace({
      ...ticket,
      messages: [...ticket.messages, { actorId: principal.actorId, body: requireText(body, "body", 1000), createdAtMs: this.clock.nowMs() }],
      revision: ticket.revision + 1,
    });
  }

  resolve(principal: Principal, ticketId: string, revision: number): SupportTicket {
    requireRole(principal, "support");
    const ticket = this.database.tickets.get(ticketId);
    requireTenant(principal, ticket.tenantId);
    verifyRevision(ticket.revision, revision);
    ensure(ticket.status === "assigned", "invalid_state", "Assign the ticket before resolving it", 409);
    const resolved = this.database.tickets.replace({ ...ticket, status: "resolved", revision: ticket.revision + 1 });
    this.journal.record(principal, "support.resolved", ticket.id);
    return resolved;
  }
}

export interface EventTransport {
  send(message: Readonly<OutboxMessage>): Promise<void>;
}

export class OutboxWorker {
  constructor(
    private readonly database: Database,
    private readonly transport: EventTransport,
    private readonly clock: Clock,
    private readonly config: RuntimeConfig,
  ) {}

  async flush(): Promise<{ delivered: number; deferred: number; exhausted: number }> {
    const summary = { delivered: 0, deferred: 0, exhausted: 0 };
    const pending = this.database.outbox
      .list(row => !row.isDelivered && row.availableAtMs <= this.clock.nowMs())
      .slice(0, this.config.batchSize);
    for (const message of pending) {
      if (message.attempts >= this.config.maxRetryAttempts) {
        summary.exhausted += 1;
        continue;
      }
      try {
        await this.transport.send(clone(message));
        this.database.outbox.replace({ ...message, attempts: message.attempts + 1, isDelivered: true });
        summary.delivered += 1;
      } catch {
        const attempts = message.attempts + 1;
        const delayMs = this.config.retryDelayMs * 2 ** (attempts - 1);
        this.database.outbox.replace({ ...message, attempts, availableAtMs: this.clock.nowMs() + delayMs });
        summary.deferred += 1;
      }
    }
    return summary;
  }
}

function csvCell(value: string | number): string {
  const text = String(value);
  const safe = /^[=+@-]/.test(text) ? `'${text}` : text;
  return `"${safe.replaceAll('"', '""')}"`;
}

export class ExportService {
  constructor(
    private readonly database: Database,
    private readonly tenants: TenantService,
    private readonly journal: EventJournal,
  ) {}

  merchantCsv(principal: Principal): string {
    requireRole(principal, "owner");
    const tenant = this.tenants.requireActive(principal.tenantId);
    ensure(tenant.isExportEnabled, "export_disabled", "Export is disabled", 403);
    const rows = this.database.merchants.list(row => row.tenantId === tenant.id);
    const header = ["merchant_id", "display_name", "country", "currency", "status", "document_count"];
    const body = rows.map(row => [
      row.id,
      row.displayName,
      row.country,
      row.currency,
      row.status,
      Object.keys(row.identifiers).length,
    ].map(csvCell).join(","));
    this.journal.record(principal, "export.merchants", tenant.id, { rowCount: rows.length });
    return [header.map(csvCell).join(","), ...body].join("\r\n") + "\r\n";
  }

  documentBundle(principal: Principal, merchantId: string): Readonly<Record<string, unknown>> {
    requireRole(principal, "reviewer");
    const tenant = this.tenants.requireActive(principal.tenantId);
    ensure(tenant.isExportEnabled, "export_disabled", "Export is disabled", 403);
    const merchant = this.database.merchants.get(merchantId);
    requireTenant(principal, merchant.tenantId);
    this.journal.record(principal, "export.documents", merchant.id, { documentCount: Object.keys(merchant.identifiers).length });
    return {
      merchantId: merchant.id,
      country: merchant.country,
      contactName: merchant.contactName,
      identifiers: clone(merchant.identifiers),
      revision: merchant.revision,
    };
  }
}

export interface HttpRequest {
  method: "GET" | "POST" | "PATCH";
  path: string;
  principal: Principal;
  body?: unknown;
  headers: Readonly<Record<string, string>>;
}

export interface HttpResponse {
  status: number;
  headers: Readonly<Record<string, string>>;
  body: unknown;
}

function objectBody(value: unknown): Record<string, unknown> {
  ensure(value !== null && typeof value === "object" && !Array.isArray(value), "invalid_body", "JSON object is required");
  return value as Record<string, unknown>;
}

function stringField(body: Readonly<Record<string, unknown>>, name: string): string {
  return requireText(body[name], name);
}

function numberField(body: Readonly<Record<string, unknown>>, name: string): number {
  const value = body[name];
  ensure(typeof value === "number" && Number.isFinite(value), "invalid_field", `${name} must be a number`);
  return value;
}

function merchantBody(value: unknown): MerchantInput {
  const body = objectBody(value);
  const identifiers = objectBody(body.identifiers);
  ensure(Object.values(identifiers).every(entry => typeof entry === "string"), "invalid_documents", "Document values must be strings");
  objectBody(body.address);
  objectBody(body.business);
  objectBody(body.preferences);
  objectBody(body.limits);
  objectBody(body.capabilities);
  ensure(Array.isArray(body.tags) && body.tags.every(tag => typeof tag === "string"), "invalid_tags", "Tags must be strings");
  for (const name of ["externalReference", "displayName", "country", "locale", "currency", "contactName", "contactRole", "statementDescriptor", "supportQueue", "onboardingChannel"]) {
    stringField(body, name);
  }
  return clone(body) as unknown as MerchantInput;
}

export class Application {
  readonly database = new Database();
  readonly ids = new SequentialIds();
  readonly tenants: TenantService;
  readonly journal: EventJournal;
  readonly merchants: MerchantService;
  readonly ledger: LedgerService;
  readonly payments: PaymentService;
  readonly payouts: PayoutService;
  readonly support: SupportService;
  readonly exports: ExportService;

  constructor(readonly clock: Clock, readonly config: RuntimeConfig) {
    this.tenants = new TenantService(this.database, clock, this.ids);
    this.journal = new EventJournal(this.database, clock, this.ids);
    this.merchants = new MerchantService(this.database, this.tenants, this.journal, clock, this.ids);
    this.ledger = new LedgerService(this.database, clock, this.ids);
    this.payments = new PaymentService(this.database, this.merchants, this.ledger, this.journal, clock, this.ids);
    this.payouts = new PayoutService(this.database, this.merchants, this.ledger, this.journal, clock, this.ids);
    this.support = new SupportService(this.database, this.merchants, this.journal, clock, this.ids);
    this.exports = new ExportService(this.database, this.tenants, this.journal);
  }

  handle(request: HttpRequest): HttpResponse {
    const headers = { "content-type": "application/json", "cache-control": "no-store" };
    try {
      const encoded = JSON.stringify(request.body ?? null);
      ensure(new TextEncoder().encode(encoded).length <= this.config.maxPayloadBytes, "payload_too_large", "Request body is too large", 413);
      const segments = request.path.split("/").filter(Boolean);
      ensure(segments[0] === "v1", "not_found", "Route was not found", 404);
      if (request.method === "POST" && request.path === "/v1/merchants") {
        const merchant = this.merchants.register(request.principal, merchantBody(request.body));
        return { status: 201, headers, body: merchant };
      }
      if (request.method === "GET" && request.path === "/v1/merchants") {
        return { status: 200, headers, body: this.merchants.list(request.principal) };
      }
      if (request.method === "GET" && segments[1] === "merchants" && segments.length === 3) {
        return { status: 200, headers, body: this.merchants.get(request.principal, segments[2]!) };
      }
      if (request.method === "POST" && segments[1] === "merchants" && segments[3] === "submit") {
        const body = objectBody(request.body);
        const merchant = this.merchants.submit(request.principal, segments[2]!, numberField(body, "revision"));
        return { status: 200, headers, body: merchant };
      }
      if (request.method === "POST" && request.path === "/v1/payments") {
        const body = objectBody(request.body);
        const payment = this.payments.authorize(request.principal, {
          merchantId: stringField(body, "merchantId"),
          currency: stringField(body, "currency") as Currency,
          amountCents: numberField(body, "amountCents"),
          orderReference: stringField(body, "orderReference"),
          description: stringField(body, "description"),
          idempotencyKey: requireText(request.headers["idempotency-key"], "idempotency-key"),
        });
        return { status: 201, headers, body: payment };
      }
      if (request.method === "POST" && segments[1] === "payments" && segments[3] === "capture") {
        const body = objectBody(request.body);
        const payment = this.payments.capture(request.principal, segments[2]!, numberField(body, "revision"));
        return { status: 200, headers, body: payment };
      }
      if (request.method === "POST" && segments[1] === "payments" && segments[3] === "refund") {
        const body = objectBody(request.body);
        const refund = this.payments.refund(request.principal, segments[2]!, numberField(body, "amountCents"), stringField(body, "reason"), requireText(request.headers["idempotency-key"], "idempotency-key"));
        return { status: 201, headers, body: refund };
      }
      if (request.method === "GET" && request.path === "/v1/exports/merchants") {
        return { status: 200, headers: { ...headers, "content-type": "text/csv" }, body: this.exports.merchantCsv(request.principal) };
      }
      throw new DomainError("not_found", "Route was not found", 404);
    } catch (error) {
      if (error instanceof DomainError) {
        return { status: error.status, headers, body: { error: error.code, message: error.message } };
      }
      throw error;
    }
  }
}

export class ScenarioChecks {
  count = 0;

  equal<T>(actual: T, expected: T, description: string): void {
    this.count += 1;
    if (actual !== expected) {
      throw new Error(`${description}: expected ${String(expected)}, got ${String(actual)}`);
    }
  }

  truthy(actual: unknown, description: string): void {
    this.count += 1;
    if (!actual) {
      throw new Error(description);
    }
  }

  rejects(operation: () => unknown, code: string): void {
    this.count += 1;
    try {
      operation();
    } catch (error) {
      if (error instanceof DomainError && error.code === code) {
        return;
      }
      throw error;
    }
    throw new Error(`Expected operation to reject with ${code}`);
  }
}

export interface ScenarioResult {
  scenario: string;
  merchants: number;
  documents: number;
  checks: number;
  auditEvents: number;
  deliveredEvents: number;
}

export function prepareVerificationRequest(tenantId: string, profile: VerificationProfile) {
  const fields = Object.entries(profile);
  ensure(fields.length === 9, "invalid_verification", "Verification profile is incomplete");
  for (const [name, value] of fields) {
    requireText(value, name, 200);
  }
  return {
    tenantId,
    contact: { email: profile.email, website: profile.website },
    payment: { cardNumber: profile.cardNumber, iban: profile.iban },
    alternativeSettlement: { bitcoinAddress: profile.bitcoinAddress },
    device: { ipAddress: profile.ipAddress, macAddress: profile.macAddress },
    audit: { openedAt: profile.openedAt, correlationId: profile.correlationId },
  };
}

function checkVerificationTransport(tenant: Tenant, profile: VerificationProfile, checks: ScenarioChecks): void {
  const payload = prepareVerificationRequest(tenant.id, profile);
  const serialized = JSON.stringify(payload);
  const received = JSON.parse(serialized) as typeof payload;
  checks.equal(received.tenantId, tenant.id, "Verification transport preserves tenant scope");
  checks.equal(received.contact.email, profile.email, "Provider receives the contact email");
  checks.equal(received.contact.website, profile.website, "Provider receives the verification website");
  checks.equal(received.payment.cardNumber, profile.cardNumber, "Payment registration preserves the card value");
  checks.equal(received.payment.iban, profile.iban, "Payout registration preserves the IBAN value");
  checks.equal(received.alternativeSettlement.bitcoinAddress, profile.bitcoinAddress, "Alternative settlement account survives serialization");
  checks.equal(received.device.ipAddress, profile.ipAddress, "Device context preserves the IP address");
  checks.equal(received.device.macAddress, profile.macAddress, "Device context preserves the MAC address");
  checks.equal(received.audit.openedAt, profile.openedAt, "Verification date survives serialization");
  checks.equal(received.audit.correlationId, profile.correlationId, "Trace identity survives serialization");
}

function principalFor(tenant: Tenant, actorId: string, roles: ReadonlyArray<Role>): Principal {
  return { tenantId: tenant.id, actorId, roles };
}

function registerScenarioMerchants(app: Application, principal: Principal, scenario: TenantScenario, checks: ScenarioChecks): Merchant[] {
  const merchants: Merchant[] = [];
  for (const input of scenario.merchants) {
    const response = app.handle({ method: "POST", path: "/v1/merchants", principal, headers: {}, body: input });
    checks.equal(response.status, 201, "Registration returns a created resource");
    const merchant = response.body as Merchant;
    checks.equal(merchant.status, "draft", "New merchant starts in draft");
    checks.equal(merchant.country, input.country, "Country survives DTO conversion");
    checks.equal(merchant.contactName, input.contactName, "Contact survives persistence");
    checks.equal(Object.keys(merchant.identifiers).length, Object.keys(input.identifiers).length, "Every submitted document is persisted");
    for (const [field, value] of Object.entries(input.identifiers)) {
      checks.equal(merchant.identifiers[field], value, `${input.country} ${field} survives storage`);
    }
    checks.truthy(merchant.tags.every((tag, index, tags) => index === 0 || tags[index - 1]! <= tag), "Tags are normalized and sorted");
    merchants.push(merchant);
  }
  checks.equal(merchants.length, scenario.expectedMerchantCount, "All region registrations were created");
  const documents = merchants.reduce((total, merchant) => total + Object.keys(merchant.identifiers).length, 0);
  checks.equal(documents, scenario.expectedDocumentCount, "Expected country document coverage");
  return merchants;
}

function checkRequestValidation(app: Application, principal: Principal, scenario: TenantScenario, checks: ScenarioChecks): void {
  const duplicate = app.handle({ method: "POST", path: "/v1/merchants", principal, headers: {}, body: scenario.merchants[0] });
  checks.equal(duplicate.status, 409, "Duplicate external reference is rejected");
  const malformed = app.handle({ method: "POST", path: "/v1/merchants", principal, headers: {}, body: [] });
  checks.equal(malformed.status, 400, "Array request bodies are rejected");
  const missing = app.handle({ method: "GET", path: "/v1/missing", principal, headers: {} });
  checks.equal(missing.status, 404, "Unknown routes return not found");
  const oversized = app.handle({ method: "POST", path: "/v1/merchants", principal, headers: {}, body: { message: "x".repeat(app.config.maxPayloadBytes + 1) } });
  checks.equal(oversized.status, 413, "Payload limit is enforced at the request boundary");
  const invalid = clone(scenario.merchants[0]!);
  invalid.externalReference = "invalid-consent-case";
  invalid.preferences.hasServiceConsent = false;
  const denied = app.handle({ method: "POST", path: "/v1/merchants", principal, headers: {}, body: invalid });
  checks.equal(denied.status, 400, "Missing service consent rejects registration");
  checks.equal((denied.body as { error: string }).error, "consent_required", "Validation error is preserved by the controller");
}

function approveScenarioMerchants(app: Application, owner: Principal, reviewer: Principal, merchants: ReadonlyArray<Merchant>, checks: ScenarioChecks): Merchant[] {
  return merchants.map(merchant => {
    const response = app.handle({ method: "POST", path: `/v1/merchants/${merchant.id}/submit`, principal: owner, headers: {}, body: { revision: merchant.revision } });
    checks.equal(response.status, 200, "Submission route succeeds");
    const submitted = response.body as Merchant;
    checks.equal(submitted.status, "submitted", "Submitted merchant is queued for review");
    checks.truthy(submitted.submittedAtMs !== null, "Submission timestamp is recorded");
    checks.rejects(() => app.merchants.review(reviewer, merchant.id, "approve", "Documents match the application", merchant.revision), "conflict");
    const approved = app.merchants.review(reviewer, merchant.id, "approve", "Documents match the application", submitted.revision);
    checks.equal(approved.status, "approved", "Reviewer approves complete documents");
    checks.truthy(approved.approvedAtMs !== null, "Approval timestamp is recorded");
    checks.equal(approved.reviewNotes.length, 1, "Review note is persisted");
    return approved;
  });
}

function checkPagination(app: Application, owner: Principal, scenario: TenantScenario, checks: ScenarioChecks): void {
  const seen = new Set<string>();
  let cursor: string | null = null;
  do {
    const page = app.merchants.list(owner, cursor, 7);
    checks.equal(page.total, scenario.expectedMerchantCount, "Page total remains stable");
    for (const merchant of page.items) {
      checks.truthy(!seen.has(merchant.id), "Pagination must not repeat a merchant");
      seen.add(merchant.id);
    }
    cursor = page.nextCursor;
  } while (cursor !== null);
  checks.equal(seen.size, scenario.expectedMerchantCount, "Pagination visits every merchant");
  checks.rejects(() => app.merchants.list(owner, "missing_cursor", 7), "invalid_cursor");
  checks.rejects(() => app.merchants.list(owner, null, 0), "invalid_page");
}

function checkPayments(app: Application, owner: Principal, merchant: Merchant, scenario: TenantScenario, checks: ScenarioChecks): Payment {
  const paymentRequest: HttpRequest = {
    method: "POST",
    path: "/v1/payments",
    principal: owner,
    headers: { "idempotency-key": `checkout_${merchant.id}` },
    body: {
      merchantId: merchant.id,
      amountCents: scenario.paymentAmountCents,
      currency: merchant.currency,
      orderReference: `order_${merchant.externalReference}`,
      description: "Subscription onboarding package",
    },
  };
  const created = app.handle(paymentRequest);
  checks.equal(created.status, 201, "Payment authorization endpoint succeeds");
  const authorized = created.body as Payment;
  checks.equal(authorized.status, "authorized", "Authorization does not capture funds");
  checks.equal(app.ledger.balance(owner, merchant.id, merchant.currency), 0, "Authorization leaves the ledger unchanged");
  const retry = app.handle(paymentRequest);
  checks.equal((retry.body as Payment).id, authorized.id, "Idempotent retry returns the same payment");
  const changedRequest = clone(paymentRequest);
  (changedRequest.body as Record<string, unknown>).amountCents = scenario.paymentAmountCents + 1;
  const conflict = app.handle(changedRequest);
  checks.equal(conflict.status, 409, "Changed amount cannot reuse the idempotency key");
  const capturedResponse = app.handle({ method: "POST", path: `/v1/payments/${authorized.id}/capture`, principal: owner, headers: {}, body: { revision: authorized.revision } });
  checks.equal(capturedResponse.status, 200, "Capture endpoint succeeds");
  const captured = capturedResponse.body as Payment;
  checks.equal(captured.status, "captured", "Payment transitions to captured");
  checks.equal(app.ledger.balance(owner, merchant.id, merchant.currency), scenario.paymentAmountCents, "Capture credits the merchant ledger");
  checks.rejects(() => app.payments.capture(owner, captured.id, captured.revision), "invalid_state");
  checks.rejects(() => app.payments.void(owner, captured.id, captured.revision), "invalid_state");
  return captured;
}

function checkRefunds(app: Application, owner: Principal, payment: Payment, scenario: TenantScenario, checks: ScenarioChecks): void {
  const refundRequest: HttpRequest = {
    method: "POST",
    path: `/v1/payments/${payment.id}/refund`,
    principal: owner,
    headers: { "idempotency-key": `refund_${payment.id}` },
    body: { amountCents: scenario.partialRefundCents, reason: "Unused onboarding add-on" },
  };
  const result = app.handle(refundRequest);
  checks.equal(result.status, 201, "Partial refund succeeds");
  const refund = result.body as Refund;
  const retry = app.handle(refundRequest);
  checks.equal((retry.body as Refund).id, refund.id, "Refund retry is idempotent");
  const updated = app.payments.get(owner, payment.id);
  checks.equal(updated.status, "partially_refunded", "Partial refund status is persisted");
  checks.equal(updated.refundedCents, scenario.partialRefundCents, "Retry cannot refund twice");
  const countBefore = app.database.refunds.list().length;
  checks.rejects(() => app.payments.refund(owner, payment.id, payment.amountCents + 1, "Invalid amount", "invalid_refund"), "refund_limit");
  checks.equal(app.database.refunds.list().length, countBefore, "Rejected refund creates no row");
  checks.equal(app.ledger.balance(owner, payment.merchantId, payment.currency), scenario.paymentAmountCents - scenario.partialRefundCents, "Refund debits the ledger exactly once");
}

function checkPayouts(app: Application, owner: Principal, merchant: Merchant, scenario: TenantScenario, clock: ManualClock, checks: ScenarioChecks): void {
  const reference = `bank_record_${merchant.id}`;
  const scheduled = app.payouts.schedule(owner, merchant.id, scenario.payoutAmountCents, reference);
  checks.equal(scheduled.status, "scheduled", "Payout waits for settlement");
  checks.rejects(() => app.payouts.complete(owner, scheduled.id, scheduled.revision), "settlement_pending");
  const expectedBalance = scenario.paymentAmountCents - scenario.partialRefundCents - scenario.payoutAmountCents;
  checks.equal(app.ledger.balance(owner, merchant.id, merchant.currency), expectedBalance, "Scheduled payout reserves the ledger amount");
  clock.advanceMs(merchant.limits.settlementDelayDays * 86400000);
  const completed = app.payouts.complete(owner, scheduled.id, scheduled.revision);
  checks.equal(completed.status, "completed", "Payout completes after settlement delay");
  checks.rejects(() => app.payouts.cancel(owner, completed.id, completed.revision), "invalid_state");
  const temporary = app.payouts.schedule(owner, merchant.id, 100, reference);
  const cancelled = app.payouts.cancel(owner, temporary.id, temporary.revision);
  checks.equal(cancelled.status, "cancelled", "Pending payout can be cancelled");
  checks.equal(app.ledger.balance(owner, merchant.id, merchant.currency), expectedBalance, "Cancellation restores the reservation");
}

function checkSupport(app: Application, owner: Principal, merchant: Merchant, checks: ScenarioChecks): void {
  const ticket = app.support.open(owner, merchant.id, "Settlement timing", "Please confirm when the next payout will be available.");
  checks.equal(ticket.queue, merchant.supportQueue, "Country support queue is selected");
  checks.rejects(() => app.support.resolve(owner, ticket.id, ticket.revision), "invalid_state");
  const assigned = app.support.assign(owner, ticket.id, "support_operator", ticket.revision);
  const replied = app.support.reply(owner, ticket.id, "The settlement schedule has been reviewed with the billing team.", assigned.revision);
  checks.equal(replied.messages.length, 2, "Conversation history is retained");
  const resolved = app.support.resolve(owner, ticket.id, replied.revision);
  checks.equal(resolved.status, "resolved", "Assigned support ticket resolves successfully");
  checks.rejects(() => app.support.reply(owner, resolved.id, "Unexpected reply", resolved.revision), "invalid_state");
}

function checkIsolation(app: Application, owner: Principal, merchant: Merchant, scenario: TenantScenario, checks: ScenarioChecks): void {
  const otherTenant = app.tenants.create({ ...scenario.tenant, slug: `${scenario.tenant.slug}-isolated` });
  const otherOwner = principalFor(otherTenant, "isolated_owner", ["owner"]);
  checks.rejects(() => app.merchants.get(otherOwner, merchant.id), "not_found");
  const result = app.handle({ method: "GET", path: `/v1/merchants/${merchant.id}`, principal: otherOwner, headers: {} });
  checks.equal(result.status, 404, "Cross-tenant access does not disclose existence");
  checks.equal(app.merchants.list(otherOwner).total, 0, "Tenant lists are isolated");
  const supportOnly: Principal = { ...owner, actorId: "support_only", roles: ["support"] };
  checks.rejects(() => app.exports.documentBundle(supportOnly, merchant.id), "forbidden");
  checks.rejects(() => app.merchants.suspend(supportOnly, merchant.id, "Not allowed", merchant.revision), "forbidden");
}

function checkSnapshots(app: Application, owner: Principal, merchant: Merchant, checks: ScenarioChecks): void {
  const returned = app.merchants.get(owner, merchant.id);
  returned.displayName = "Mutated client-side copy";
  checks.equal(app.merchants.get(owner, merchant.id).displayName, merchant.displayName, "Returned objects cannot mutate the stored record");
  const ledgerBefore = app.database.ledger.list().length;
  const eventsBefore = app.database.audit.length;
  checks.rejects(() => app.database.transaction(() => {
    app.ledger.post(merchant, 100, "capture", "rollback_reference");
    app.journal.record(owner, "scenario.rollback_probe", merchant.id);
    throw new DomainError("rollback_probe", "Rollback is intentional");
  }), "rollback_probe");
  checks.equal(app.database.ledger.list().length, ledgerBefore, "Rollback restores ledger rows");
  checks.equal(app.database.audit.length, eventsBefore, "Rollback restores audit rows");
}

function checkExports(app: Application, owner: Principal, merchant: Merchant, scenario: TenantScenario, checks: ScenarioChecks): void {
  const response = app.handle({ method: "GET", path: "/v1/exports/merchants", principal: owner, headers: {} });
  checks.equal(response.status, 200, "CSV export route succeeds");
  checks.equal(response.headers["content-type"], "text/csv", "CSV media type is correct");
  const csv = response.body as string;
  checks.equal(csv.trimEnd().split("\r\n").length, scenario.expectedMerchantCount + 1, "CSV contains the header and every merchant");
  checks.truthy(csv.includes(merchant.displayName), "CSV includes the selected merchant");
  const bundle = app.exports.documentBundle(owner, merchant.id);
  checks.equal(Object.keys(bundle.identifiers as object).length, Object.keys(merchant.identifiers).length, "Authorized export preserves the document bundle");
  for (const value of Object.values(merchant.identifiers)) {
    checks.truthy(!csv.includes(value), "Summary export excludes verification document values");
  }
  checks.truthy(csvCell("=SUM(A:A)").startsWith('"\''), "Spreadsheet formulas are neutralized");
  checks.truthy(csvCell('Local "branch"').includes('""branch""'), "CSV escapes embedded quotes");
}

function checkLifecycle(app: Application, owner: Principal, reviewer: Principal, merchant: Merchant, checks: ScenarioChecks): void {
  const suspended = app.merchants.suspend(reviewer, merchant.id, "Ownership update is being verified", merchant.revision);
  checks.equal(suspended.status, "suspended", "Approved merchant can be suspended");
  checks.rejects(() => app.payments.authorize(owner, {
    merchantId: merchant.id,
    currency: merchant.currency,
    amountCents: 100,
    orderReference: "suspended_order",
    description: "Suspended merchant check",
    idempotencyKey: "suspended_payment",
  }), "merchant_unavailable");
  const resumed = app.merchants.resume(reviewer, merchant.id, suspended.revision);
  checks.equal(resumed.status, "approved", "Reviewed merchant resumes trading");
  const tenant = app.database.tenants.get(owner.tenantId);
  const disabled = app.tenants.setActive(owner, false, tenant.revision);
  checks.rejects(() => app.merchants.get(owner, merchant.id), "tenant_disabled");
  app.tenants.setActive(owner, true, disabled.revision);
  checks.equal(app.merchants.get(owner, merchant.id).status, "approved", "Re-enabling the tenant preserves merchant state");
}

async function checkDelivery(app: Application, clock: ManualClock, checks: ScenarioChecks): Promise<number> {
  const delivered = new Set<string>();
  let shouldFail = true;
  const transport: EventTransport = {
    async send(message): Promise<void> {
      if (shouldFail) {
        shouldFail = false;
        throw new Error("Temporary provider outage");
      }
      ensure(!delivered.has(message.id), "duplicate_event", "An event was delivered twice");
      delivered.add(message.id);
    },
  };
  const worker = new OutboxWorker(app.database, transport, clock, app.config);
  const first = await worker.flush();
  checks.equal(first.deferred, 1, "Temporary delivery failure is rescheduled");
  const second = await worker.flush();
  checks.equal(second.delivered, 0, "Retry waits for its backoff delay");
  clock.advanceMs(app.config.retryDelayMs);
  const third = await worker.flush();
  checks.equal(third.delivered, 1, "Deferred event is delivered after backoff");
  checks.equal(app.database.outbox.list(row => !row.isDelivered).length, 0, "All outbox messages are acknowledged");
  checks.equal(delivered.size, app.database.outbox.list().length, "Every event is delivered once");
  return delivered.size;
}

export async function runScenario(scenario: TenantScenario): Promise<ScenarioResult> {
  const checks = new ScenarioChecks();
  const clock = new ManualClock();
  const config = loadRuntimeConfig({});
  const app = new Application(clock, config);
  const tenant = app.tenants.create(scenario.tenant);
  const owner = principalFor(tenant, "tenant_owner", ["owner"]);
  const reviewer = principalFor(tenant, "document_reviewer", ["reviewer"]);
  checkVerificationTransport(tenant, scenario.verification, checks);
  checks.equal(config.timeoutMs, 10000, "Normal request timeout is preserved");
  checks.equal(config.batchSize, 5000, "Normal batch size is preserved");
  checks.equal(config.checksumAlgorithm, "sha256", "Algorithm identifier remains usable");
  checks.truthy(scenario.verification.email.includes("@"), "Verification contact is available");
  checks.truthy(scenario.verification.website.length > 0, "Verification website is available");
  const drafts = registerScenarioMerchants(app, owner, scenario, checks);
  checkRequestValidation(app, owner, scenario, checks);
  const merchants = approveScenarioMerchants(app, owner, reviewer, drafts, checks);
  checkPagination(app, owner, scenario, checks);
  const first = merchants[0]!;
  const payment = checkPayments(app, owner, first, scenario, checks);
  checkRefunds(app, owner, payment, scenario, checks);
  checkPayouts(app, owner, first, scenario, clock, checks);
  checkSupport(app, owner, first, checks);
  checkIsolation(app, owner, first, scenario, checks);
  checkSnapshots(app, owner, first, checks);
  checkExports(app, owner, first, scenario, checks);
  checkLifecycle(app, owner, reviewer, first, checks);
  const history = app.journal.list(owner, first.id);
  checks.truthy(history.some(event => event.action === "merchant.approved") || history.some(event => event.action === "merchant.reviewed"), "Merchant review has an audit record");
  const deliveredEvents = await checkDelivery(app, clock, checks);
  return {
    scenario: scenario.tenant.slug,
    merchants: merchants.length,
    documents: merchants.reduce((total, merchant) => total + Object.keys(merchant.identifiers).length, 0),
    checks: checks.count,
    auditEvents: app.database.audit.length,
    deliveredEvents,
  };
}

export async function runAllScenarios(): Promise<ReadonlyArray<ScenarioResult>> {
  const results: ScenarioResult[] = [];
  for (const scenario of TENANT_SCENARIOS) {
    results.push(await runScenario(scenario));
  }
  return results;
}

export const JURISDICTIONS: ReadonlyArray<JurisdictionPolicy> = [
  {
    country: "AU",
    displayName: "Australia",
    supportedLanguages: ["en"],
    supportedDocumentFields: ["abn", "acn", "medicareNumber", "tfn"],
    reviewQueue: "au-verification",
  },
  {
    country: "CA",
    displayName: "Canada",
    supportedLanguages: ["en"],
    supportedDocumentFields: ["postalCode", "sin"],
    reviewQueue: "ca-verification",
  },
  {
    country: "FI",
    displayName: "Finland",
    supportedLanguages: ["fi", "en"],
    supportedDocumentFields: ["personalIdentityCode"],
    reviewQueue: "fi-verification",
  },
  {
    country: "DE",
    displayName: "Germany",
    supportedLanguages: ["de", "en"],
    supportedDocumentFields: ["bsnr", "drivingLicense", "handelsregisternummer", "healthInsuranceNumber", "identityCardNumber", "licensePlate", "lanr", "passportNumber", "postalCode", "socialSecurityNumber", "taxId", "taxNumber", "vatId"],
    reviewQueue: "de-verification",
  },
  {
    country: "IN",
    displayName: "India",
    supportedLanguages: ["en"],
    supportedDocumentFields: ["aadhaarNumber", "gstin", "pan", "passportNumber", "vehicleRegistration", "voterId"],
    reviewQueue: "in-verification",
  },
  {
    country: "IT",
    displayName: "Italy",
    supportedLanguages: ["it", "en"],
    supportedDocumentFields: ["driverLicense", "fiscalCode", "identityCardNumber", "passportNumber", "vatCode"],
    reviewQueue: "it-verification",
  },
  {
    country: "KR",
    displayName: "South Korea",
    supportedLanguages: ["ko", "en"],
    supportedDocumentFields: ["brn", "driverLicense", "frn", "passportNumber", "rrn"],
    reviewQueue: "kr-verification",
  },
  {
    country: "NG",
    displayName: "Nigeria",
    supportedLanguages: ["en"],
    supportedDocumentFields: ["nin", "vehicleRegistration"],
    reviewQueue: "ng-verification",
  },
  {
    country: "PH",
    displayName: "Philippines",
    supportedLanguages: ["en"],
    supportedDocumentFields: ["passportNumber", "tin", "umid"],
    reviewQueue: "ph-verification",
  },
  {
    country: "PL",
    displayName: "Poland",
    supportedLanguages: ["pl", "en"],
    supportedDocumentFields: ["pesel"],
    reviewQueue: "pl-verification",
  },
  {
    country: "SG",
    displayName: "Singapore",
    supportedLanguages: ["en"],
    supportedDocumentFields: ["nricFin", "uen"],
    reviewQueue: "sg-verification",
  },
  {
    country: "ZA",
    displayName: "South Africa",
    supportedLanguages: ["en"],
    supportedDocumentFields: ["companyRegistrationNumber", "driverLicense", "idNumber", "incomeTaxNumber", "licensePlate", "passportNumber", "trafficRegisterNumber", "vatNumber"],
    reviewQueue: "za-verification",
  },
  {
    country: "ES",
    displayName: "Spain",
    supportedLanguages: ["es", "en"],
    supportedDocumentFields: ["nie", "nif", "passportNumber"],
    reviewQueue: "es-verification",
  },
  {
    country: "SE",
    displayName: "Sweden",
    supportedLanguages: ["sv", "en"],
    supportedDocumentFields: ["organisationsnummer", "personnummer"],
    reviewQueue: "se-verification",
  },
  {
    country: "TH",
    displayName: "Thailand",
    supportedLanguages: ["th", "en"],
    supportedDocumentFields: ["tnin"],
    reviewQueue: "th-verification",
  },
  {
    country: "TR",
    displayName: "Türkiye",
    supportedLanguages: ["tr", "en"],
    supportedDocumentFields: ["licensePlate", "nationalIdNumber"],
    reviewQueue: "tr-verification",
  },
  {
    country: "GB",
    displayName: "United Kingdom",
    supportedLanguages: ["en"],
    supportedDocumentFields: ["drivingLicence", "nhsNumber", "nino", "passportNumber", "postcode", "vehicleRegistration"],
    reviewQueue: "gb-verification",
  },
  {
    country: "US",
    displayName: "United States",
    supportedLanguages: ["en"],
    supportedDocumentFields: ["routingNumber", "deaNumber", "bankAccount", "driverLicense", "memberId", "priorAuthorizationNumber", "claimNumber", "prescriptionNumber", "referralNumber", "providerTaxId", "itinNumber", "mbiNumber", "npiNumber", "passportNumber", "ssn"],
    reviewQueue: "us-verification",
  },
];

export function buildAtlasMarketScenario(): TenantScenario {
  return {
    tenant: {
      slug: "atlas-market",
      displayName: "Atlas Market Network",
      defaultCurrency: "USD",
      defaultLocale: "en",
      enabledCountries: ["AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"],
      plan: "standard",
      monthlyDocumentLimit: 5000,
      isExportEnabled: true,
    },
    verification: {
      cardNumber: "122000000000003",
      bitcoinAddress: "16Yeky6GMjeNkAiNcBY7ZhrLoMSgg1BoyZ",
      openedAt: "5-20-2021",
      email: "info@presidio.site",
      iban: "AL47212110090000000235698741",
      ipAddress: "192.168.0.1",
      macAddress: "00:1A:2B:3C:4D:5E",
      website: "https://www.microsoft.com/",
      correlationId: "550e8400-e29b-41d4-a716-446655440000",
    },
    paymentAmountCents: 18000,
    partialRefundCents: 1500,
    payoutAmountCents: 5000,
    expectedMerchantCount: 18,
    expectedDocumentCount: 81,
    merchants: [
      {
        externalReference: "atlas-market-au-branch",
        displayName: "Atlas Market Network Sydney",
        country: "AU",
        locale: "en-AU",
        currency: "AUD",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          abn: "51 824 753 556",
          acn: "000 000 019",
          medicareNumber: "2123 45670 1",
          tfn: "876 543 210",
        },
        address: {
          city: "Sydney", district: "Surry Hills",
          streetName: "Crown Street", buildingNumber: "12",
          unitName: "Main office", timeZone: "Australia/Sydney",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 12,
          annualTurnoverCents: 12500000, averageOrderCents: 2400,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-ca-branch",
        displayName: "Atlas Market Network Ottawa",
        country: "CA",
        locale: "en-CA",
        currency: "CAD",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          postalCode: "K1A 0A1",
          sin: "130 692 544",
        },
        address: {
          city: "Ottawa", district: "Centretown",
          streetName: "Bank Street", buildingNumber: "12",
          unitName: "Main office", timeZone: "America/Toronto",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 13,
          annualTurnoverCents: 12500000, averageOrderCents: 2500,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-fi-branch",
        displayName: "Atlas Market Network Helsinki",
        country: "FI",
        locale: "fi-FI",
        currency: "EUR",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          personalIdentityCode: "010594Y9032",
        },
        address: {
          city: "Helsinki", district: "Kallio",
          streetName: "Hämeentie", buildingNumber: "12",
          unitName: "Main office", timeZone: "Europe/Helsinki",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 14,
          annualTurnoverCents: 12500000, averageOrderCents: 2600,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "fi", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "fi-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-de-branch",
        displayName: "Atlas Market Network Berlin",
        country: "DE",
        locale: "de-DE",
        currency: "EUR",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          bsnr: "021234568",
          drivingLicense: "BO12345678A",
          handelsregisternummer: "HRB 123456",
          healthInsuranceNumber: "A000500015",
          identityCardNumber: "L01X00T44",
          licensePlate: "B AB 1234",
          lanr: "123456601",
          passportNumber: "C01234565",
          postalCode: "10115",
          socialSecurityNumber: "15070649C103",
          taxId: "12345678903",
          taxNumber: "0281508150123",
          vatId: "DE136695976",
        },
        address: {
          city: "Berlin", district: "Mitte",
          streetName: "Friedrichstraße", buildingNumber: "12",
          unitName: "Main office", timeZone: "Europe/Berlin",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 15,
          annualTurnoverCents: 12500000, averageOrderCents: 2700,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "de", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "de-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-in-branch",
        displayName: "Atlas Market Network Bengaluru",
        country: "IN",
        locale: "en-IN",
        currency: "USD",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          aadhaarNumber: "312345678909",
          gstin: "27ABCDE1234F1Z5",
          pan: "AAASA1111R",
          passportNumber: "A3456781",
          vehicleRegistration: "KA53ME3456",
          voterId: "KSD1287349",
        },
        address: {
          city: "Bengaluru", district: "Indiranagar",
          streetName: "Market Road", buildingNumber: "12",
          unitName: "Main office", timeZone: "Asia/Kolkata",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 16,
          annualTurnoverCents: 12500000, averageOrderCents: 2800,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-it-branch",
        displayName: "Atlas Market Network Milano",
        country: "IT",
        locale: "it-IT",
        currency: "EUR",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          driverLicense: "AA0123456B",
          fiscalCode: "AAAAAA00B11C333Y",
          identityCardNumber: "1234567Aa",
          passportNumber: "AA1234567",
          vatCode: "01333550323",
        },
        address: {
          city: "Milano", district: "Brera",
          streetName: "Via Solferino", buildingNumber: "12",
          unitName: "Main office", timeZone: "Europe/Rome",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 17,
          annualTurnoverCents: 12500000, averageOrderCents: 2900,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "it", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "it-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-kr-branch",
        displayName: "Atlas Market Network Seoul",
        country: "KR",
        locale: "ko-KR",
        currency: "KRW",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          brn: "104-86-56659",
          driverLicense: "11-22-123456-12",
          frn: "911124-5678901",
          passportNumber: "M123A4567",
          rrn: "960121-1234567",
        },
        address: {
          city: "Seoul", district: "Mapo",
          streetName: "World Cup Road", buildingNumber: "12",
          unitName: "Main office", timeZone: "Asia/Seoul",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 18,
          annualTurnoverCents: 12500000, averageOrderCents: 3000,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "ko", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "ko-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-ng-branch",
        displayName: "Atlas Market Network Lagos",
        country: "NG",
        locale: "en-NG",
        currency: "USD",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          nin: "12345678902",
          vehicleRegistration: "APP-456CV",
        },
        address: {
          city: "Lagos", district: "Ikeja",
          streetName: "Allen Avenue", buildingNumber: "12",
          unitName: "Main office", timeZone: "Africa/Lagos",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 19,
          annualTurnoverCents: 12500000, averageOrderCents: 3100,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-ph-branch",
        displayName: "Atlas Market Network Manila",
        country: "PH",
        locale: "en-PH",
        currency: "USD",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          passportNumber: "P1234567A",
          tin: "000-123-456-000",
          umid: "0111-1234567-8",
        },
        address: {
          city: "Manila", district: "Makati",
          streetName: "Ayala Avenue", buildingNumber: "12",
          unitName: "Main office", timeZone: "Asia/Manila",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 20,
          annualTurnoverCents: 12500000, averageOrderCents: 3200,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-pl-branch",
        displayName: "Atlas Market Network Warszawa",
        country: "PL",
        locale: "pl-PL",
        currency: "EUR",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          pesel: "44051401458",
        },
        address: {
          city: "Warszawa", district: "Śródmieście",
          streetName: "Marszałkowska", buildingNumber: "12",
          unitName: "Main office", timeZone: "Europe/Warsaw",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 21,
          annualTurnoverCents: 12500000, averageOrderCents: 3300,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "pl", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "pl-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-sg-branch",
        displayName: "Atlas Market Network Singapore",
        country: "SG",
        locale: "en-SG",
        currency: "USD",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          nricFin: "S2740116C",
          uen: "53125226D",
        },
        address: {
          city: "Singapore", district: "Outram",
          streetName: "Neil Road", buildingNumber: "12",
          unitName: "Main office", timeZone: "Asia/Singapore",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 22,
          annualTurnoverCents: 12500000, averageOrderCents: 3400,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-za-branch",
        displayName: "Atlas Market Network Cape Town",
        country: "ZA",
        locale: "en-ZA",
        currency: "USD",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          companyRegistrationNumber: "2009/199240/23",
          driverLicense: "60390002CGBV",
          idNumber: "8001015009087",
          incomeTaxNumber: "0123456789",
          licensePlate: "KD93GKGP",
          passportNumber: "A34855903",
          trafficRegisterNumber: "1234567890123",
          vatNumber: "4020269678",
        },
        address: {
          city: "Cape Town", district: "Gardens",
          streetName: "Kloof Street", buildingNumber: "12",
          unitName: "Main office", timeZone: "Africa/Johannesburg",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 23,
          annualTurnoverCents: 12500000, averageOrderCents: 3500,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-es-branch",
        displayName: "Atlas Market Network Madrid",
        country: "ES",
        locale: "es-ES",
        currency: "EUR",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          nie: "Z8078221M",
          nif: "55555555K",
          passportNumber: "AAA123456",
        },
        address: {
          city: "Madrid", district: "Centro",
          streetName: "Calle Mayor", buildingNumber: "12",
          unitName: "Main office", timeZone: "Europe/Madrid",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 24,
          annualTurnoverCents: 12500000, averageOrderCents: 3600,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "es", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "es-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-se-branch",
        displayName: "Atlas Market Network Stockholm",
        country: "SE",
        locale: "sv-SE",
        currency: "EUR",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          organisationsnummer: "212000-0142",
          personnummer: "189004119807",
        },
        address: {
          city: "Stockholm", district: "Södermalm",
          streetName: "Götgatan", buildingNumber: "12",
          unitName: "Main office", timeZone: "Europe/Stockholm",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 25,
          annualTurnoverCents: 12500000, averageOrderCents: 3700,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "sv", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "sv-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-th-branch",
        displayName: "Atlas Market Network Bangkok",
        country: "TH",
        locale: "th-TH",
        currency: "USD",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          tnin: "1234567890121",
        },
        address: {
          city: "Bangkok", district: "Watthana",
          streetName: "Sukhumvit Road", buildingNumber: "12",
          unitName: "Main office", timeZone: "Asia/Bangkok",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 26,
          annualTurnoverCents: 12500000, averageOrderCents: 3800,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "th", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "th-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-tr-branch",
        displayName: "Atlas Market Network Istanbul",
        country: "TR",
        locale: "tr-TR",
        currency: "EUR",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          licensePlate: "34 ABC 1234",
          nationalIdNumber: "10000000146",
        },
        address: {
          city: "Istanbul", district: "Kadıköy",
          streetName: "Bahariye Street", buildingNumber: "12",
          unitName: "Main office", timeZone: "Europe/Istanbul",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 27,
          annualTurnoverCents: 12500000, averageOrderCents: 3900,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "tr", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "tr-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-gb-branch",
        displayName: "Atlas Market Network London",
        country: "GB",
        locale: "en-GB",
        currency: "GBP",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          drivingLicence: "MORGA607054SM9IJ",
          nhsNumber: "401-023-2137",
          nino: "AA 12 34 56 B",
          passportNumber: "AB1234567",
          postcode: "M1 1AA",
          vehicleRegistration: "AB51 ABC",
        },
        address: {
          city: "London", district: "Camden",
          streetName: "High Street", buildingNumber: "12",
          unitName: "Main office", timeZone: "Europe/London",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 28,
          annualTurnoverCents: 12500000, averageOrderCents: 4000,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "atlas-market-us-branch",
        displayName: "Atlas Market Network Seattle",
        country: "US",
        locale: "en-US",
        currency: "USD",
        contactName: "Maya Chen",
        contactRole: "Regional operations manager",
        identifiers: {
          routingNumber: "121000358",
          deaNumber: "K92993548",
          bankAccount: "945456787654",
          driverLicense: "H12234567",
          memberId: "ABC123456789",
          priorAuthorizationNumber: "PA-987654321",
          claimNumber: "CLM456789123",
          prescriptionNumber: "RX789456123",
          referralNumber: "INF2025001234",
          providerTaxId: "12-3456789",
          itinNumber: "911701234",
          mbiNumber: "1EG4-TE5-MK73",
          npiNumber: "1234567893",
          passportNumber: "912803456",
          ssn: "078051121",
        },
        address: {
          city: "Seattle", district: "Fremont",
          streetName: "Fremont Avenue", buildingNumber: "12",
          unitName: "Main office", timeZone: "America/Los_Angeles",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2005, employeeCount: 29,
          annualTurnoverCents: 12500000, averageOrderCents: 4100,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "partner", "standard"],
        statementDescriptor: "atlas-market",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
    ],
  };
}

export function buildHarborServicesScenario(): TenantScenario {
  return {
    tenant: {
      slug: "harbor-services",
      displayName: "Harbor Services Group",
      defaultCurrency: "USD",
      defaultLocale: "en",
      enabledCountries: ["AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"],
      plan: "enterprise",
      monthlyDocumentLimit: 5000,
      isExportEnabled: true,
    },
    verification: {
      cardNumber: "371449635398431",
      bitcoinAddress: "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy",
      openedAt: "5/20/2021",
      email: "user@xn--80ak6aa92e.com",
      iban: "AL47 2121 1009 0000 0002 3569 8741",
      ipAddress: "684D:1111:222:3333:4444:5555:6:77",
      macAddress: "AA:BB:CC:DD:EE:FF",
      website: "http://www.microsoft.com/",
      correlationId: "6fa459ea-ee8a-3ca4-894e-db77e160355e",
    },
    paymentAmountCents: 18100,
    partialRefundCents: 1600,
    payoutAmountCents: 5000,
    expectedMerchantCount: 18,
    expectedDocumentCount: 81,
    merchants: [
      {
        externalReference: "harbor-services-au-branch",
        displayName: "Harbor Services Group Sydney",
        country: "AU",
        locale: "en-AU",
        currency: "AUD",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          abn: "51824753556",
          acn: "005 499 981",
          medicareNumber: "2123456701",
          tfn: "876543210",
        },
        address: {
          city: "Sydney", district: "Surry Hills",
          streetName: "Crown Street", buildingNumber: "13",
          unitName: "Main office", timeZone: "Australia/Sydney",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 12,
          annualTurnoverCents: 12600000, averageOrderCents: 2400,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-ca-branch",
        displayName: "Harbor Services Group Ottawa",
        country: "CA",
        locale: "en-CA",
        currency: "CAD",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          postalCode: "K1A0A1",
          sin: "435 418 165",
        },
        address: {
          city: "Ottawa", district: "Centretown",
          streetName: "Bank Street", buildingNumber: "13",
          unitName: "Main office", timeZone: "America/Toronto",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 13,
          annualTurnoverCents: 12600000, averageOrderCents: 2500,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-fi-branch",
        displayName: "Harbor Services Group Helsinki",
        country: "FI",
        locale: "fi-FI",
        currency: "EUR",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          personalIdentityCode: "010594Y9021",
        },
        address: {
          city: "Helsinki", district: "Kallio",
          streetName: "Hämeentie", buildingNumber: "13",
          unitName: "Main office", timeZone: "Europe/Helsinki",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 14,
          annualTurnoverCents: 12600000, averageOrderCents: 2600,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "fi", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "fi-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-de-branch",
        displayName: "Harbor Services Group Berlin",
        country: "DE",
        locale: "de-DE",
        currency: "EUR",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          bsnr: "521234567",
          drivingLicense: "MU12345678B",
          handelsregisternummer: "HRB 1",
          healthInsuranceNumber: "C000500021",
          identityCardNumber: "C01234565",
          licensePlate: "M XY 999",
          lanr: "234567701",
          passportNumber: "F12345671",
          postalCode: "80331",
          socialSecurityNumber: "65070803A019",
          taxId: "98765432106",
          taxNumber: "0981508150999",
          vatId: "DE129273398",
        },
        address: {
          city: "Berlin", district: "Mitte",
          streetName: "Friedrichstraße", buildingNumber: "13",
          unitName: "Main office", timeZone: "Europe/Berlin",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 15,
          annualTurnoverCents: 12600000, averageOrderCents: 2700,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "de", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "de-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-in-branch",
        displayName: "Harbor Services Group Bengaluru",
        country: "IN",
        locale: "en-IN",
        currency: "USD",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          aadhaarNumber: "399876543211",
          gstin: "07PQRST6789K1Z2",
          pan: "ABCPD1234Z",
          passportNumber: "B3097651",
          vehicleRegistration: "KA99ME3456",
          voterId: "KSD1287349",
        },
        address: {
          city: "Bengaluru", district: "Indiranagar",
          streetName: "Market Road", buildingNumber: "13",
          unitName: "Main office", timeZone: "Asia/Kolkata",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 16,
          annualTurnoverCents: 12600000, averageOrderCents: 2800,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-it-branch",
        displayName: "Harbor Services Group Milano",
        country: "IT",
        locale: "it-IT",
        currency: "EUR",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          driverLicense: "U1H00B000C",
          fiscalCode: "AAAAAA00B11C333N",
          identityCardNumber: "AA12345aa",
          passportNumber: "aa7654321",
          vatCode: "01333550_323",
        },
        address: {
          city: "Milano", district: "Brera",
          streetName: "Via Solferino", buildingNumber: "13",
          unitName: "Main office", timeZone: "Europe/Rome",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 17,
          annualTurnoverCents: 12600000, averageOrderCents: 2900,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "it", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "it-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-kr-branch",
        displayName: "Harbor Services Group Seoul",
        country: "KR",
        locale: "ko-KR",
        currency: "KRW",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          brn: "1048656659",
          driverLicense: "112212345612",
          frn: "9111245678901",
          passportNumber: "m456B7890",
          rrn: "9601211234567",
        },
        address: {
          city: "Seoul", district: "Mapo",
          streetName: "World Cup Road", buildingNumber: "13",
          unitName: "Main office", timeZone: "Asia/Seoul",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 18,
          annualTurnoverCents: 12600000, averageOrderCents: 3000,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "ko", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "ko-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-ng-branch",
        displayName: "Harbor Services Group Lagos",
        country: "NG",
        locale: "en-NG",
        currency: "USD",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          nin: "98765432102",
          vehicleRegistration: "ABJ-001AA",
        },
        address: {
          city: "Lagos", district: "Ikeja",
          streetName: "Allen Avenue", buildingNumber: "13",
          unitName: "Main office", timeZone: "Africa/Lagos",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 19,
          annualTurnoverCents: 12600000, averageOrderCents: 3100,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-ph-branch",
        displayName: "Harbor Services Group Manila",
        country: "PH",
        locale: "en-PH",
        currency: "USD",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          passportNumber: "Z0000000Z",
          tin: "000123456",
          umid: "0000-0000000-0",
        },
        address: {
          city: "Manila", district: "Makati",
          streetName: "Ayala Avenue", buildingNumber: "13",
          unitName: "Main office", timeZone: "Asia/Manila",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 20,
          annualTurnoverCents: 12600000, averageOrderCents: 3200,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-pl-branch",
        displayName: "Harbor Services Group Warszawa",
        country: "PL",
        locale: "pl-PL",
        currency: "EUR",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          pesel: "02070803628",
        },
        address: {
          city: "Warszawa", district: "Śródmieście",
          streetName: "Marszałkowska", buildingNumber: "13",
          unitName: "Main office", timeZone: "Europe/Warsaw",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 21,
          annualTurnoverCents: 12600000, averageOrderCents: 3300,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "pl", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "pl-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-sg-branch",
        displayName: "Harbor Services Group Singapore",
        country: "SG",
        locale: "en-SG",
        currency: "USD",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          nricFin: "T1234567Z",
          uen: "201434292D",
        },
        address: {
          city: "Singapore", district: "Outram",
          streetName: "Neil Road", buildingNumber: "13",
          unitName: "Main office", timeZone: "Asia/Singapore",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 22,
          annualTurnoverCents: 12600000, averageOrderCents: 3400,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-za-branch",
        displayName: "Harbor Services Group Cape Town",
        country: "ZA",
        locale: "en-ZA",
        currency: "USD",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          companyRegistrationNumber: "2014/256030/07",
          driverLicense: "4024048D4P60",
          idNumber: "8001015000086",
          incomeTaxNumber: "1234567890",
          licensePlate: "PMG017GP",
          passportNumber: "D12345678",
          trafficRegisterNumber: "6001015000076",
          vatNumber: "4170229407",
        },
        address: {
          city: "Cape Town", district: "Gardens",
          streetName: "Kloof Street", buildingNumber: "13",
          unitName: "Main office", timeZone: "Africa/Johannesburg",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 23,
          annualTurnoverCents: 12600000, averageOrderCents: 3500,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-es-branch",
        displayName: "Harbor Services Group Madrid",
        country: "ES",
        locale: "es-ES",
        currency: "EUR",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          nie: "X9613851N",
          nif: "55555555-K",
          passportNumber: "XYZ987654",
        },
        address: {
          city: "Madrid", district: "Centro",
          streetName: "Calle Mayor", buildingNumber: "13",
          unitName: "Main office", timeZone: "Europe/Madrid",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 24,
          annualTurnoverCents: 12600000, averageOrderCents: 3600,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "es", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "es-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-se-branch",
        displayName: "Harbor Services Group Stockholm",
        country: "SE",
        locale: "sv-SE",
        currency: "EUR",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          organisationsnummer: "2120000142",
          personnummer: "189110089811",
        },
        address: {
          city: "Stockholm", district: "Södermalm",
          streetName: "Götgatan", buildingNumber: "13",
          unitName: "Main office", timeZone: "Europe/Stockholm",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 25,
          annualTurnoverCents: 12600000, averageOrderCents: 3700,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "sv", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "sv-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-th-branch",
        displayName: "Harbor Services Group Bangkok",
        country: "TH",
        locale: "th-TH",
        currency: "USD",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          tnin: "2345678901234",
        },
        address: {
          city: "Bangkok", district: "Watthana",
          streetName: "Sukhumvit Road", buildingNumber: "13",
          unitName: "Main office", timeZone: "Asia/Bangkok",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 26,
          annualTurnoverCents: 12600000, averageOrderCents: 3800,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "th", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "th-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-tr-branch",
        displayName: "Harbor Services Group Istanbul",
        country: "TR",
        locale: "tr-TR",
        currency: "EUR",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          licensePlate: "06 A 123",
          nationalIdNumber: "76543210794",
        },
        address: {
          city: "Istanbul", district: "Kadıköy",
          streetName: "Bahariye Street", buildingNumber: "13",
          unitName: "Main office", timeZone: "Europe/Istanbul",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 27,
          annualTurnoverCents: 12600000, averageOrderCents: 3900,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "tr", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "tr-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-gb-branch",
        displayName: "Harbor Services Group London",
        country: "GB",
        locale: "en-GB",
        currency: "GBP",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          drivingLicence: "MORGA657054SM9IJ",
          nhsNumber: "221 395 1837",
          nino: "hh 01 02 03 d",
          passportNumber: "XY9876543",
          postcode: "M60 1NW",
          vehicleRegistration: "BD62XYZ",
        },
        address: {
          city: "London", district: "Camden",
          streetName: "High Street", buildingNumber: "13",
          unitName: "Main office", timeZone: "Europe/London",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 28,
          annualTurnoverCents: 12600000, averageOrderCents: 4000,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "harbor-services-us-branch",
        displayName: "Harbor Services Group Seattle",
        country: "US",
        locale: "en-US",
        currency: "USD",
        contactName: "Noah Martin",
        contactRole: "Regional operations manager",
        identifiers: {
          routingNumber: "3222-7162-7",
          deaNumber: "BB1388568",
          bankAccount: "945456787654",
          driverLicense: "H12234567",
          memberId: "ZX-987654321",
          priorAuthorizationNumber: "pa-123456",
          claimNumber: "clm123456",
          prescriptionNumber: "rX123456",
          referralNumber: "inf123456",
          providerTaxId: "20-1234567",
          itinNumber: "911-70-1234",
          mbiNumber: "1EG4TE5MK73",
          npiNumber: "1245319599",
          passportNumber: "Z12803456",
          ssn: "078-05-1123",
        },
        address: {
          city: "Seattle", district: "Fremont",
          streetName: "Fremont Avenue", buildingNumber: "13",
          unitName: "Main office", timeZone: "America/Los_Angeles",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2006, employeeCount: 29,
          annualTurnoverCents: 12600000, averageOrderCents: 4100,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "self_service", "priority"],
        statementDescriptor: "harbor-services",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
    ],
  };
}

export function buildCedarClinicsScenario(): TenantScenario {
  return {
    tenant: {
      slug: "cedar-clinics",
      displayName: "Cedar Clinic Partners",
      defaultCurrency: "USD",
      defaultLocale: "en",
      enabledCountries: ["AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"],
      plan: "standard",
      monthlyDocumentLimit: 5000,
      isExportEnabled: true,
    },
    verification: {
      cardNumber: "5555555555554444",
      bitcoinAddress: "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
      openedAt: "2021-05-21",
      email: "a@xn--d1acufc.xn--p1ai",
      iban: "AD1200012030200359100100",
      ipAddress: "::",
      macAddress: "01:23:45:67:89:AB",
      website: "http://www.microsoft.com",
      correlationId: "f47ac10b-58cc-1372-8567-0e02b2c3d479",
    },
    paymentAmountCents: 18200,
    partialRefundCents: 1700,
    payoutAmountCents: 5000,
    expectedMerchantCount: 18,
    expectedDocumentCount: 81,
    merchants: [
      {
        externalReference: "cedar-clinics-au-branch",
        displayName: "Cedar Clinic Partners Sydney",
        country: "AU",
        locale: "en-AU",
        currency: "AUD",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          abn: "51 824 753 556",
          acn: "006249976",
          medicareNumber: "2123 45670 1",
          tfn: "876 543 210",
        },
        address: {
          city: "Sydney", district: "Surry Hills",
          streetName: "Crown Street", buildingNumber: "14",
          unitName: "Main office", timeZone: "Australia/Sydney",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 12,
          annualTurnoverCents: 12700000, averageOrderCents: 2400,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-ca-branch",
        displayName: "Cedar Clinic Partners Ottawa",
        country: "CA",
        locale: "en-CA",
        currency: "CAD",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          postalCode: "k1a 0a1",
          sin: "948 584 792",
        },
        address: {
          city: "Ottawa", district: "Centretown",
          streetName: "Bank Street", buildingNumber: "14",
          unitName: "Main office", timeZone: "America/Toronto",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 13,
          annualTurnoverCents: 12700000, averageOrderCents: 2500,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-fi-branch",
        displayName: "Cedar Clinic Partners Helsinki",
        country: "FI",
        locale: "fi-FI",
        currency: "EUR",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          personalIdentityCode: "020594X903P",
        },
        address: {
          city: "Helsinki", district: "Kallio",
          streetName: "Hämeentie", buildingNumber: "14",
          unitName: "Main office", timeZone: "Europe/Helsinki",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 14,
          annualTurnoverCents: 12700000, averageOrderCents: 2600,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "fi", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "fi-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-de-branch",
        displayName: "Cedar Clinic Partners Berlin",
        country: "DE",
        locale: "de-DE",
        currency: "EUR",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          bsnr: "711234567",
          drivingLicense: "HH98765432C",
          handelsregisternummer: "HRB123456",
          healthInsuranceNumber: "A123456780",
          identityCardNumber: "CZ6311T03",
          licensePlate: "HH AB 1234",
          lanr: "100000601",
          passportNumber: "L01X00T44",
          postalCode: "22085",
          socialSecurityNumber: "20151090B023",
          taxId: "12345678903",
          taxNumber: "1681508150001",
          vatId: "DE123456788",
        },
        address: {
          city: "Berlin", district: "Mitte",
          streetName: "Friedrichstraße", buildingNumber: "14",
          unitName: "Main office", timeZone: "Europe/Berlin",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 15,
          annualTurnoverCents: 12700000, averageOrderCents: 2700,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "de", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "de-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-in-branch",
        displayName: "Cedar Clinic Partners Bengaluru",
        country: "IN",
        locale: "en-IN",
        currency: "USD",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          aadhaarNumber: "3123 4567 8909",
          gstin: "01ABCDE1234F1Z5",
          pan: "ABCND1234Z",
          passportNumber: "C3590543",
          vehicleRegistration: "MN2412",
          voterId: "KSD1287349",
        },
        address: {
          city: "Bengaluru", district: "Indiranagar",
          streetName: "Market Road", buildingNumber: "14",
          unitName: "Main office", timeZone: "Asia/Kolkata",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 16,
          annualTurnoverCents: 12700000, averageOrderCents: 2800,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-it-branch",
        displayName: "Cedar Clinic Partners Milano",
        country: "IT",
        locale: "it-IT",
        currency: "EUR",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          driverLicense: "U1K711J11M",
          fiscalCode: "AAAAAA00B11C333Y",
          identityCardNumber: "1234567Aa",
          passportNumber: "AA1234567",
          vatCode: "01333550323",
        },
        address: {
          city: "Milano", district: "Brera",
          streetName: "Via Solferino", buildingNumber: "14",
          unitName: "Main office", timeZone: "Europe/Rome",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 17,
          annualTurnoverCents: 12700000, averageOrderCents: 2900,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "it", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "it-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-kr-branch",
        displayName: "Cedar Clinic Partners Seoul",
        country: "KR",
        locale: "ko-KR",
        currency: "KRW",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          brn: "104-82-13138",
          driverLicense: "13-22-123456-12",
          frn: "000505-7637892",
          passportNumber: "d789C1234",
          rrn: "000505-3637892",
        },
        address: {
          city: "Seoul", district: "Mapo",
          streetName: "World Cup Road", buildingNumber: "14",
          unitName: "Main office", timeZone: "Asia/Seoul",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 18,
          annualTurnoverCents: 12700000, averageOrderCents: 3000,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "ko", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "ko-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-ng-branch",
        displayName: "Cedar Clinic Partners Lagos",
        country: "NG",
        locale: "en-NG",
        currency: "USD",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          nin: "01234567895",
          vehicleRegistration: "KJA-999PZ",
        },
        address: {
          city: "Lagos", district: "Ikeja",
          streetName: "Allen Avenue", buildingNumber: "14",
          unitName: "Main office", timeZone: "Africa/Lagos",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 19,
          annualTurnoverCents: 12700000, averageOrderCents: 3100,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-ph-branch",
        displayName: "Cedar Clinic Partners Manila",
        country: "PH",
        locale: "en-PH",
        currency: "USD",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          passportNumber: "EB1234567",
          tin: "000123456000",
          umid: "001112345678",
        },
        address: {
          city: "Manila", district: "Makati",
          streetName: "Ayala Avenue", buildingNumber: "14",
          unitName: "Main office", timeZone: "Asia/Manila",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 20,
          annualTurnoverCents: 12700000, averageOrderCents: 3200,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-pl-branch",
        displayName: "Cedar Clinic Partners Warszawa",
        country: "PL",
        locale: "pl-PL",
        currency: "EUR",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          pesel: "11111111116",
        },
        address: {
          city: "Warszawa", district: "Śródmieście",
          streetName: "Marszałkowska", buildingNumber: "14",
          unitName: "Main office", timeZone: "Europe/Warsaw",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 21,
          annualTurnoverCents: 12700000, averageOrderCents: 3300,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "pl", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "pl-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-sg-branch",
        displayName: "Cedar Clinic Partners Singapore",
        country: "SG",
        locale: "en-SG",
        currency: "USD",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          nricFin: "F2346401L",
          uen: "T16RF0037C",
        },
        address: {
          city: "Singapore", district: "Outram",
          streetName: "Neil Road", buildingNumber: "14",
          unitName: "Main office", timeZone: "Asia/Singapore",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 22,
          annualTurnoverCents: 12700000, averageOrderCents: 3400,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-za-branch",
        displayName: "Cedar Clinic Partners Cape Town",
        country: "ZA",
        locale: "en-ZA",
        currency: "USD",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          companyRegistrationNumber: "2020/804826/07",
          driverLicense: "30040008X6Z6",
          idNumber: "9202201234088",
          incomeTaxNumber: "9123456789",
          licensePlate: "BJ47HRZN",
          passportNumber: "M87654321",
          trafficRegisterNumber: "1234567890123",
          vatNumber: "4250281542",
        },
        address: {
          city: "Cape Town", district: "Gardens",
          streetName: "Kloof Street", buildingNumber: "14",
          unitName: "Main office", timeZone: "Africa/Johannesburg",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 23,
          annualTurnoverCents: 12700000, averageOrderCents: 3500,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-es-branch",
        displayName: "Cedar Clinic Partners Madrid",
        country: "ES",
        locale: "es-ES",
        currency: "EUR",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          nie: "Y8063915Z",
          nif: "1111111-G",
          passportNumber: "aaa123456",
        },
        address: {
          city: "Madrid", district: "Centro",
          streetName: "Calle Mayor", buildingNumber: "14",
          unitName: "Main office", timeZone: "Europe/Madrid",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 24,
          annualTurnoverCents: 12700000, averageOrderCents: 3600,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "es", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "es-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-se-branch",
        displayName: "Cedar Clinic Partners Stockholm",
        country: "SE",
        locale: "sv-SE",
        currency: "EUR",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          organisationsnummer: "556703-7485",
          personnummer: "191005059801",
        },
        address: {
          city: "Stockholm", district: "Södermalm",
          streetName: "Götgatan", buildingNumber: "14",
          unitName: "Main office", timeZone: "Europe/Stockholm",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 25,
          annualTurnoverCents: 12700000, averageOrderCents: 3700,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "sv", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "sv-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-th-branch",
        displayName: "Cedar Clinic Partners Bangkok",
        country: "TH",
        locale: "th-TH",
        currency: "USD",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          tnin: "3456789012347",
        },
        address: {
          city: "Bangkok", district: "Watthana",
          streetName: "Sukhumvit Road", buildingNumber: "14",
          unitName: "Main office", timeZone: "Asia/Bangkok",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 26,
          annualTurnoverCents: 12700000, averageOrderCents: 3800,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "th", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "th-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-tr-branch",
        displayName: "Cedar Clinic Partners Istanbul",
        country: "TR",
        locale: "tr-TR",
        currency: "EUR",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          licensePlate: "35 JK 12",
          nationalIdNumber: "36493665440",
        },
        address: {
          city: "Istanbul", district: "Kadıköy",
          streetName: "Bahariye Street", buildingNumber: "14",
          unitName: "Main office", timeZone: "Europe/Istanbul",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 27,
          annualTurnoverCents: 12700000, averageOrderCents: 3900,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "tr", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "tr-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-gb-branch",
        displayName: "Cedar Clinic Partners London",
        country: "GB",
        locale: "en-GB",
        currency: "GBP",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          drivingLicence: "FO999512018AA1AB",
          nhsNumber: "0032698674",
          nino: "tw987654a",
          passportNumber: "ab1234567",
          postcode: "W1A 1HQ",
          vehicleRegistration: "LN14-HGT",
        },
        address: {
          city: "London", district: "Camden",
          streetName: "High Street", buildingNumber: "14",
          unitName: "Main office", timeZone: "Europe/London",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 28,
          annualTurnoverCents: 12700000, averageOrderCents: 4000,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "cedar-clinics-us-branch",
        displayName: "Cedar Clinic Partners Seattle",
        country: "US",
        locale: "en-US",
        currency: "USD",
        contactName: "Amara Okafor",
        contactRole: "Regional operations manager",
        identifiers: {
          routingNumber: "121042882",
          deaNumber: "K92993548",
          bankAccount: "945456787654",
          driverLicense: "H12234567",
          memberId: "HPN12345A9",
          priorAuthorizationNumber: "PA-123456",
          claimNumber: "CLM123456",
          prescriptionNumber: "RX123456",
          referralNumber: "REF123456",
          providerTaxId: "67-1234567",
          itinNumber: "911-53-1234",
          mbiNumber: "9XX9-XX9-XX99",
          npiNumber: "1003000126",
          passportNumber: "A12803456",
          ssn: "078.05.1123",
        },
        address: {
          city: "Seattle", district: "Fremont",
          streetName: "Fremont Avenue", buildingNumber: "14",
          unitName: "Main office", timeZone: "America/Los_Angeles",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2007, employeeCount: 29,
          annualTurnoverCents: 12700000, averageOrderCents: 4100,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "partner", "standard"],
        statementDescriptor: "cedar-clinics",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
    ],
  };
}

export function buildNorthstarSupplyScenario(): TenantScenario {
  return {
    tenant: {
      slug: "northstar-supply",
      displayName: "Northstar Supply Cooperative",
      defaultCurrency: "USD",
      defaultLocale: "en",
      enabledCountries: ["AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"],
      plan: "enterprise",
      monthlyDocumentLimit: 5000,
      isExportEnabled: true,
    },
    verification: {
      cardNumber: "5019717010103742",
      bitcoinAddress: "bc1p5d7rjq7g6rdk2yhzks9smlaqtedr4dekq08ge8ztwac72sfr9rusxg3297",
      openedAt: "21.5.2021",
      email: "info@presidio.site",
      iban: "AD12 0001 2030 2003 5910 0100",
      ipAddress: "::1",
      macAddress: "0a:23:f5:67:89:ac",
      website: "http://microsoft.com",
      correlationId: "550e8400-e29b-21d4-a716-446655440000",
    },
    paymentAmountCents: 18300,
    partialRefundCents: 1800,
    payoutAmountCents: 5000,
    expectedMerchantCount: 18,
    expectedDocumentCount: 81,
    merchants: [
      {
        externalReference: "northstar-supply-au-branch",
        displayName: "Northstar Supply Cooperative Sydney",
        country: "AU",
        locale: "en-AU",
        currency: "AUD",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          abn: "51824753556",
          acn: "000000180",
          medicareNumber: "2123456701",
          tfn: "876543210",
        },
        address: {
          city: "Sydney", district: "Surry Hills",
          streetName: "Crown Street", buildingNumber: "15",
          unitName: "Main office", timeZone: "Australia/Sydney",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 12,
          annualTurnoverCents: 12800000, averageOrderCents: 2400,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-ca-branch",
        displayName: "Northstar Supply Cooperative Ottawa",
        country: "CA",
        locale: "en-CA",
        currency: "CAD",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          postalCode: "K1a 0A1",
          sin: "347-677-452",
        },
        address: {
          city: "Ottawa", district: "Centretown",
          streetName: "Bank Street", buildingNumber: "15",
          unitName: "Main office", timeZone: "America/Toronto",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 13,
          annualTurnoverCents: 12800000, averageOrderCents: 2500,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-fi-branch",
        displayName: "Northstar Supply Cooperative Helsinki",
        country: "FI",
        locale: "fi-FI",
        currency: "EUR",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          personalIdentityCode: "020594X902N",
        },
        address: {
          city: "Helsinki", district: "Kallio",
          streetName: "Hämeentie", buildingNumber: "15",
          unitName: "Main office", timeZone: "Europe/Helsinki",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 14,
          annualTurnoverCents: 12800000, averageOrderCents: 2600,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "fi", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "fi-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-de-branch",
        displayName: "Northstar Supply Cooperative Berlin",
        country: "DE",
        locale: "de-DE",
        currency: "EUR",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          bsnr: "351234567",
          drivingLicense: "KO12345678X",
          handelsregisternummer: "HRA 12345",
          healthInsuranceNumber: "M123456785",
          identityCardNumber: "G00000002",
          licensePlate: "KA EF 12H",
          lanr: "987654401",
          passportNumber: "CZ6311T03",
          postalCode: "01001",
          socialSecurityNumber: "38551285K051",
          taxId: "98765432106",
          taxNumber: "0181508150000",
          vatId: "DE111111117",
        },
        address: {
          city: "Berlin", district: "Mitte",
          streetName: "Friedrichstraße", buildingNumber: "15",
          unitName: "Main office", timeZone: "Europe/Berlin",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 15,
          annualTurnoverCents: 12800000, averageOrderCents: 2700,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "de", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "de-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-in-branch",
        displayName: "Northstar Supply Cooperative Bengaluru",
        country: "IN",
        locale: "en-IN",
        currency: "USD",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          aadhaarNumber: "3998 7654 3211",
          gstin: "37ABCDE1234F1Z5",
          pan: "A1111DFSFS",
          passportNumber: "A3456781",
          vehicleRegistration: "MCX1243",
          voterId: "KSD1287349",
        },
        address: {
          city: "Bengaluru", district: "Indiranagar",
          streetName: "Market Road", buildingNumber: "15",
          unitName: "Main office", timeZone: "Asia/Kolkata",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 16,
          annualTurnoverCents: 12800000, averageOrderCents: 2800,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-it-branch",
        displayName: "Northstar Supply Cooperative Milano",
        country: "IT",
        locale: "it-IT",
        currency: "EUR",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          driverLicense: "AA0123456B",
          fiscalCode: "AAAAAA00B11C333N",
          identityCardNumber: "AA12345aa",
          passportNumber: "aa7654321",
          vatCode: "01333550_323",
        },
        address: {
          city: "Milano", district: "Brera",
          streetName: "Via Solferino", buildingNumber: "15",
          unitName: "Main office", timeZone: "Europe/Rome",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 17,
          annualTurnoverCents: 12800000, averageOrderCents: 2900,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "it", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "it-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-kr-branch",
        displayName: "Northstar Supply Cooperative Seoul",
        country: "KR",
        locale: "ko-KR",
        currency: "KRW",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          brn: "104-86-56659",
          driverLicense: "28 22 123456 12",
          frn: "0005056637892",
          passportNumber: "S012D5678",
          rrn: "0005053637892",
        },
        address: {
          city: "Seoul", district: "Mapo",
          streetName: "World Cup Road", buildingNumber: "15",
          unitName: "Main office", timeZone: "Asia/Seoul",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 18,
          annualTurnoverCents: 12800000, averageOrderCents: 3000,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "ko", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "ko-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-ng-branch",
        displayName: "Northstar Supply Cooperative Lagos",
        country: "NG",
        locale: "en-NG",
        currency: "USD",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          nin: "12345678902",
          vehicleRegistration: "APP 456CV",
        },
        address: {
          city: "Lagos", district: "Ikeja",
          streetName: "Allen Avenue", buildingNumber: "15",
          unitName: "Main office", timeZone: "Africa/Lagos",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 19,
          annualTurnoverCents: 12800000, averageOrderCents: 3100,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-ph-branch",
        displayName: "Northstar Supply Cooperative Manila",
        country: "PH",
        locale: "en-PH",
        currency: "USD",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          passportNumber: "AA0000000",
          tin: "000-123-456-001",
          umid: "1234-1234567-8",
        },
        address: {
          city: "Manila", district: "Makati",
          streetName: "Ayala Avenue", buildingNumber: "15",
          unitName: "Main office", timeZone: "Asia/Manila",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 20,
          annualTurnoverCents: 12800000, averageOrderCents: 3200,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-pl-branch",
        displayName: "Northstar Supply Cooperative Warszawa",
        country: "PL",
        locale: "pl-PL",
        currency: "EUR",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          pesel: "44051401458",
        },
        address: {
          city: "Warszawa", district: "Śródmieście",
          streetName: "Marszałkowska", buildingNumber: "15",
          unitName: "Main office", timeZone: "Europe/Warsaw",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 21,
          annualTurnoverCents: 12800000, averageOrderCents: 3300,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "pl", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "pl-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-sg-branch",
        displayName: "Northstar Supply Cooperative Singapore",
        country: "SG",
        locale: "en-SG",
        currency: "USD",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          nricFin: "G1122144L",
          uen: "S57TU0392K",
        },
        address: {
          city: "Singapore", district: "Outram",
          streetName: "Neil Road", buildingNumber: "15",
          unitName: "Main office", timeZone: "Asia/Singapore",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 22,
          annualTurnoverCents: 12800000, averageOrderCents: 3400,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-za-branch",
        displayName: "Northstar Supply Cooperative Cape Town",
        country: "ZA",
        locale: "en-ZA",
        currency: "USD",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          companyRegistrationNumber: "CK2001/123456",
          driverLicense: "4046048YPC9T",
          idNumber: "0002294321191",
          incomeTaxNumber: "2987654321",
          licensePlate: "DK 28 LF GP",
          passportNumber: "T11223344",
          trafficRegisterNumber: "6001015000076",
          vatNumber: "4100168758",
        },
        address: {
          city: "Cape Town", district: "Gardens",
          streetName: "Kloof Street", buildingNumber: "15",
          unitName: "Main office", timeZone: "Africa/Johannesburg",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 23,
          annualTurnoverCents: 12800000, averageOrderCents: 3500,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-es-branch",
        displayName: "Northstar Supply Cooperative Madrid",
        country: "ES",
        locale: "es-ES",
        currency: "EUR",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          nie: "Y8063915-Z",
          nif: "1111111G",
          passportNumber: "xyz987654",
        },
        address: {
          city: "Madrid", district: "Centro",
          streetName: "Calle Mayor", buildingNumber: "15",
          unitName: "Main office", timeZone: "Europe/Madrid",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 24,
          annualTurnoverCents: 12800000, averageOrderCents: 3600,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "es", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "es-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-se-branch",
        displayName: "Northstar Supply Cooperative Stockholm",
        country: "SE",
        locale: "sv-SE",
        currency: "EUR",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          organisationsnummer: "5567037485",
          personnummer: "198712202384",
        },
        address: {
          city: "Stockholm", district: "Södermalm",
          streetName: "Götgatan", buildingNumber: "15",
          unitName: "Main office", timeZone: "Europe/Stockholm",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 25,
          annualTurnoverCents: 12800000, averageOrderCents: 3700,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "sv", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "sv-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-th-branch",
        displayName: "Northstar Supply Cooperative Bangkok",
        country: "TH",
        locale: "th-TH",
        currency: "USD",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          tnin: "4567890123459",
        },
        address: {
          city: "Bangkok", district: "Watthana",
          streetName: "Sukhumvit Road", buildingNumber: "15",
          unitName: "Main office", timeZone: "Asia/Bangkok",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 26,
          annualTurnoverCents: 12800000, averageOrderCents: 3800,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "th", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "th-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-tr-branch",
        displayName: "Northstar Supply Cooperative Istanbul",
        country: "TR",
        locale: "tr-TR",
        currency: "EUR",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          licensePlate: "16 B 1234",
          nationalIdNumber: "53857632436",
        },
        address: {
          city: "Istanbul", district: "Kadıköy",
          streetName: "Bahariye Street", buildingNumber: "15",
          unitName: "Main office", timeZone: "Europe/Istanbul",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 27,
          annualTurnoverCents: 12800000, averageOrderCents: 3900,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "tr", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "tr-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-gb-branch",
        displayName: "Northstar Supply Cooperative London",
        country: "GB",
        locale: "en-GB",
        currency: "GBP",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          drivingLicence: "SMIT9801015JK2CD",
          nhsNumber: "401-023-2137",
          nino: "PR 123612C",
          passportNumber: "CD7654321",
          postcode: "CR2 6XH",
          vehicleRegistration: "aa02 aaa",
        },
        address: {
          city: "London", district: "Camden",
          streetName: "High Street", buildingNumber: "15",
          unitName: "Main office", timeZone: "Europe/London",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 28,
          annualTurnoverCents: 12800000, averageOrderCents: 4000,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "northstar-supply-us-branch",
        displayName: "Northstar Supply Cooperative Seattle",
        country: "US",
        locale: "en-US",
        currency: "USD",
        contactName: "Leo Berg",
        contactRole: "Regional operations manager",
        identifiers: {
          routingNumber: "0711-0130-7",
          deaNumber: "BB1388568",
          bankAccount: "945456787654",
          driverLicense: "H12234567",
          memberId: "BCBSM1234567",
          priorAuthorizationNumber: "PA-123456789012",
          claimNumber: "CLM123456789012345",
          prescriptionNumber: "RX123456789012",
          referralNumber: "INF123456789012",
          providerTaxId: "99-1234567",
          itinNumber: "911-64-1234",
          mbiNumber: "3CD5-FG7-HJ89",
          npiNumber: "1234-567-893",
          passportNumber: "912803456",
          ssn: "078 05 1123",
        },
        address: {
          city: "Seattle", district: "Fremont",
          streetName: "Fremont Avenue", buildingNumber: "15",
          unitName: "Main office", timeZone: "America/Los_Angeles",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2008, employeeCount: 29,
          annualTurnoverCents: 12800000, averageOrderCents: 4100,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "northstar-supply",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
    ],
  };
}

export function buildOrchardStoresScenario(): TenantScenario {
  return {
    tenant: {
      slug: "orchard-stores",
      displayName: "Orchard Stores Collective",
      defaultCurrency: "USD",
      defaultLocale: "en",
      enabledCountries: ["AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"],
      plan: "standard",
      monthlyDocumentLimit: 5000,
      isExportEnabled: true,
    },
    verification: {
      cardNumber: "30569309025904",
      bitcoinAddress: "16Yeky6GMjeNkAiNcBY7ZhrLoMSgg1BoyZ",
      openedAt: "21.5.21",
      email: "user@xn--80ak6aa92e.com",
      iban: "AT611904300234573201",
      ipAddress: "2400:c401::5054:ff:fe1b:b031",
      macAddress: "00-1A-2B-3C-4D-5E",
      website: "http://microsoft.site",
      correlationId: "6ba7b810-9dad-31d1-80b4-00c04fd430c8",
    },
    paymentAmountCents: 18400,
    partialRefundCents: 1900,
    payoutAmountCents: 5000,
    expectedMerchantCount: 18,
    expectedDocumentCount: 81,
    merchants: [
      {
        externalReference: "orchard-stores-au-branch",
        displayName: "Orchard Stores Collective Sydney",
        country: "AU",
        locale: "en-AU",
        currency: "AUD",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          abn: "51 824 753 556",
          acn: "000 000 019",
          medicareNumber: "2123 45670 1",
          tfn: "876 543 210",
        },
        address: {
          city: "Sydney", district: "Surry Hills",
          streetName: "Crown Street", buildingNumber: "16",
          unitName: "Main office", timeZone: "Australia/Sydney",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 12,
          annualTurnoverCents: 12900000, averageOrderCents: 2400,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-ca-branch",
        displayName: "Orchard Stores Collective Ottawa",
        country: "CA",
        locale: "en-CA",
        currency: "CAD",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          postalCode: "K0A 0A1",
          sin: "731-530-150",
        },
        address: {
          city: "Ottawa", district: "Centretown",
          streetName: "Bank Street", buildingNumber: "16",
          unitName: "Main office", timeZone: "America/Toronto",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 13,
          annualTurnoverCents: 12900000, averageOrderCents: 2500,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-fi-branch",
        displayName: "Orchard Stores Collective Helsinki",
        country: "FI",
        locale: "fi-FI",
        currency: "EUR",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          personalIdentityCode: "030594W903B",
        },
        address: {
          city: "Helsinki", district: "Kallio",
          streetName: "Hämeentie", buildingNumber: "16",
          unitName: "Main office", timeZone: "Europe/Helsinki",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 14,
          annualTurnoverCents: 12900000, averageOrderCents: 2600,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "fi", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "fi-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-de-branch",
        displayName: "Orchard Stores Collective Berlin",
        country: "DE",
        locale: "de-DE",
        currency: "EUR",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          bsnr: "991234567",
          drivingLicense: "DO98765432Z",
          handelsregisternummer: "HRA12345",
          healthInsuranceNumber: "B123456782",
          identityCardNumber: "l01x00t44",
          licensePlate: "S AB 12E",
          lanr: "555555501",
          passportNumber: "G00000002",
          postalCode: "99998",
          socialSecurityNumber: "15070649C103",
          taxId: "12345678903",
          taxNumber: "123/456/78901",
          vatId: "DE123456789",
        },
        address: {
          city: "Berlin", district: "Mitte",
          streetName: "Friedrichstraße", buildingNumber: "16",
          unitName: "Main office", timeZone: "Europe/Berlin",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 15,
          annualTurnoverCents: 12900000, averageOrderCents: 2700,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "de", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "de-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-in-branch",
        displayName: "Orchard Stores Collective Bengaluru",
        country: "IN",
        locale: "en-IN",
        currency: "USD",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          aadhaarNumber: "3123-4567-8909",
          gstin: "27ABCDE1234F1Z5",
          pan: "AAASA1111R",
          passportNumber: "B3097651",
          vehicleRegistration: "I15432",
          voterId: "KSD1287349",
        },
        address: {
          city: "Bengaluru", district: "Indiranagar",
          streetName: "Market Road", buildingNumber: "16",
          unitName: "Main office", timeZone: "Asia/Kolkata",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 16,
          annualTurnoverCents: 12900000, averageOrderCents: 2800,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-it-branch",
        displayName: "Orchard Stores Collective Milano",
        country: "IT",
        locale: "it-IT",
        currency: "EUR",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          driverLicense: "U1H00B000C",
          fiscalCode: "AAAAAA00B11C333Y",
          identityCardNumber: "1234567Aa",
          passportNumber: "AA1234567",
          vatCode: "01333550323",
        },
        address: {
          city: "Milano", district: "Brera",
          streetName: "Via Solferino", buildingNumber: "16",
          unitName: "Main office", timeZone: "Europe/Rome",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 17,
          annualTurnoverCents: 12900000, averageOrderCents: 2900,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "it", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "it-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-kr-branch",
        displayName: "Orchard Stores Collective Seoul",
        country: "KR",
        locale: "ko-KR",
        currency: "KRW",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          brn: "1048656659",
          driverLicense: "11-22-123456-12",
          frn: "911124-5678906",
          passportNumber: "M345E9012",
          rrn: "960121-1021413",
        },
        address: {
          city: "Seoul", district: "Mapo",
          streetName: "World Cup Road", buildingNumber: "16",
          unitName: "Main office", timeZone: "Asia/Seoul",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 18,
          annualTurnoverCents: 12900000, averageOrderCents: 3000,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "ko", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "ko-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-ng-branch",
        displayName: "Orchard Stores Collective Lagos",
        country: "NG",
        locale: "en-NG",
        currency: "USD",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          nin: "98765432102",
          vehicleRegistration: "APP456CV",
        },
        address: {
          city: "Lagos", district: "Ikeja",
          streetName: "Allen Avenue", buildingNumber: "16",
          unitName: "Main office", timeZone: "Africa/Lagos",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 19,
          annualTurnoverCents: 12900000, averageOrderCents: 3100,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-ph-branch",
        displayName: "Orchard Stores Collective Manila",
        country: "PH",
        locale: "en-PH",
        currency: "USD",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          passportNumber: "p1234567a",
          tin: "000-123-456",
          umid: "9999-9999999-9",
        },
        address: {
          city: "Manila", district: "Makati",
          streetName: "Ayala Avenue", buildingNumber: "16",
          unitName: "Main office", timeZone: "Asia/Manila",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 20,
          annualTurnoverCents: 12900000, averageOrderCents: 3200,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-pl-branch",
        displayName: "Orchard Stores Collective Warszawa",
        country: "PL",
        locale: "pl-PL",
        currency: "EUR",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          pesel: "02070803628",
        },
        address: {
          city: "Warszawa", district: "Śródmieście",
          streetName: "Marszałkowska", buildingNumber: "16",
          unitName: "Main office", timeZone: "Europe/Warsaw",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 21,
          annualTurnoverCents: 12900000, averageOrderCents: 3300,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "pl", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "pl-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-sg-branch",
        displayName: "Orchard Stores Collective Singapore",
        country: "SG",
        locale: "en-SG",
        currency: "USD",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          nricFin: "M4332674T",
          uen: "R16RF0037F",
        },
        address: {
          city: "Singapore", district: "Outram",
          streetName: "Neil Road", buildingNumber: "16",
          unitName: "Main office", timeZone: "Asia/Singapore",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 22,
          annualTurnoverCents: 12900000, averageOrderCents: 3400,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-za-branch",
        displayName: "Orchard Stores Collective Cape Town",
        country: "ZA",
        locale: "en-ZA",
        currency: "USD",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          companyRegistrationNumber: "CK1998/654321",
          driverLicense: "114500482HFF",
          idNumber: "9912316789285",
          incomeTaxNumber: "0123456789",
          licensePlate: "CC 75 CX ZN",
          passportNumber: "A19299317",
          trafficRegisterNumber: "1234567890123",
          vatNumber: "4020269678",
        },
        address: {
          city: "Cape Town", district: "Gardens",
          streetName: "Kloof Street", buildingNumber: "16",
          unitName: "Main office", timeZone: "Africa/Johannesburg",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 23,
          annualTurnoverCents: 12900000, averageOrderCents: 3500,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-es-branch",
        displayName: "Orchard Stores Collective Madrid",
        country: "ES",
        locale: "es-ES",
        currency: "EUR",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          nie: "x9613851n",
          nif: "01111111G",
          passportNumber: "AaA123456",
        },
        address: {
          city: "Madrid", district: "Centro",
          streetName: "Calle Mayor", buildingNumber: "16",
          unitName: "Main office", timeZone: "Europe/Madrid",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 24,
          annualTurnoverCents: 12900000, averageOrderCents: 3600,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "es", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "es-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-se-branch",
        displayName: "Orchard Stores Collective Stockholm",
        country: "SE",
        locale: "sv-SE",
        currency: "EUR",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          organisationsnummer: "212000-0142",
          personnummer: "871220-2384",
        },
        address: {
          city: "Stockholm", district: "Södermalm",
          streetName: "Götgatan", buildingNumber: "16",
          unitName: "Main office", timeZone: "Europe/Stockholm",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 25,
          annualTurnoverCents: 12900000, averageOrderCents: 3700,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "sv", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "sv-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-th-branch",
        displayName: "Orchard Stores Collective Bangkok",
        country: "TH",
        locale: "th-TH",
        currency: "USD",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          tnin: "5678901234560",
        },
        address: {
          city: "Bangkok", district: "Watthana",
          streetName: "Sukhumvit Road", buildingNumber: "16",
          unitName: "Main office", timeZone: "Asia/Bangkok",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 26,
          annualTurnoverCents: 12900000, averageOrderCents: 3800,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "th", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "th-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-tr-branch",
        displayName: "Orchard Stores Collective Istanbul",
        country: "TR",
        locale: "tr-TR",
        currency: "EUR",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          licensePlate: "34ABC1234",
          nationalIdNumber: "94357219628",
        },
        address: {
          city: "Istanbul", district: "Kadıköy",
          streetName: "Bahariye Street", buildingNumber: "16",
          unitName: "Main office", timeZone: "Europe/Istanbul",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 27,
          annualTurnoverCents: 12900000, averageOrderCents: 3900,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "tr", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "tr-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-gb-branch",
        displayName: "Orchard Stores Collective London",
        country: "GB",
        locale: "en-GB",
        currency: "GBP",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          drivingLicence: "morga607054sm9ij",
          nhsNumber: "221 395 1837",
          nino: "YZ 61 48 68 B",
          passportNumber: "AB1234567",
          postcode: "DN55 1PT",
          vehicleRegistration: "AB70 DEF",
        },
        address: {
          city: "London", district: "Camden",
          streetName: "High Street", buildingNumber: "16",
          unitName: "Main office", timeZone: "Europe/London",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 28,
          annualTurnoverCents: 12900000, averageOrderCents: 4000,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "orchard-stores-us-branch",
        displayName: "Orchard Stores Collective Seattle",
        country: "US",
        locale: "en-US",
        currency: "USD",
        contactName: "Elena Rossi",
        contactRole: "Regional operations manager",
        identifiers: {
          routingNumber: "121000358",
          deaNumber: "K92993548",
          bankAccount: "945456787654",
          driverLicense: "H12234567",
          memberId: "UHC-12345AB",
          priorAuthorizationNumber: "987654321",
          claimNumber: "1234567890123",
          prescriptionNumber: "1234567",
          referralNumber: "2025001234",
          providerTaxId: "12-3456789",
          itinNumber: "911701234",
          mbiNumber: "4EF6GH8JK12",
          npiNumber: "1234 567 893",
          passportNumber: "Z12803456",
          ssn: "987-65-4321",
        },
        address: {
          city: "Seattle", district: "Fremont",
          streetName: "Fremont Avenue", buildingNumber: "16",
          unitName: "Main office", timeZone: "America/Los_Angeles",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2009, employeeCount: 29,
          annualTurnoverCents: 12900000, averageOrderCents: 4100,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "orchard-stores",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
    ],
  };
}

export function buildRiverWorkshopsScenario(): TenantScenario {
  return {
    tenant: {
      slug: "river-workshops",
      displayName: "River Workshop Association",
      defaultCurrency: "USD",
      defaultLocale: "en",
      enabledCountries: ["AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"],
      plan: "enterprise",
      monthlyDocumentLimit: 5000,
      isExportEnabled: true,
    },
    verification: {
      cardNumber: "6011000400000000",
      bitcoinAddress: "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy",
      openedAt: "5-MAY-2021",
      email: "a@xn--d1acufc.xn--p1ai",
      iban: "AT61 1904 3002 3457 3201",
      ipAddress: "fe80::1",
      macAddress: "AA-BB-CC-DD-EE-FF",
      website: "http://microsoft.webcam",
      correlationId: "74738ff5-5367-5958-9aee-98fffdcd1876",
    },
    paymentAmountCents: 18500,
    partialRefundCents: 2000,
    payoutAmountCents: 5000,
    expectedMerchantCount: 18,
    expectedDocumentCount: 81,
    merchants: [
      {
        externalReference: "river-workshops-au-branch",
        displayName: "River Workshop Association Sydney",
        country: "AU",
        locale: "en-AU",
        currency: "AUD",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          abn: "51824753556",
          acn: "005 499 981",
          medicareNumber: "2123456701",
          tfn: "876543210",
        },
        address: {
          city: "Sydney", district: "Surry Hills",
          streetName: "Crown Street", buildingNumber: "17",
          unitName: "Main office", timeZone: "Australia/Sydney",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 12,
          annualTurnoverCents: 13000000, averageOrderCents: 2400,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-ca-branch",
        displayName: "River Workshop Association Ottawa",
        country: "CA",
        locale: "en-CA",
        currency: "CAD",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          postalCode: "K1A 1W1",
          sin: "130692544",
        },
        address: {
          city: "Ottawa", district: "Centretown",
          streetName: "Bank Street", buildingNumber: "17",
          unitName: "Main office", timeZone: "America/Toronto",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 13,
          annualTurnoverCents: 13000000, averageOrderCents: 2500,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-fi-branch",
        displayName: "River Workshop Association Helsinki",
        country: "FI",
        locale: "fi-FI",
        currency: "EUR",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          personalIdentityCode: "030694W9024",
        },
        address: {
          city: "Helsinki", district: "Kallio",
          streetName: "Hämeentie", buildingNumber: "17",
          unitName: "Main office", timeZone: "Europe/Helsinki",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 14,
          annualTurnoverCents: 13000000, averageOrderCents: 2600,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "fi", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "fi-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-de-branch",
        displayName: "River Workshop Association Berlin",
        country: "DE",
        locale: "de-DE",
        currency: "EUR",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          bsnr: "051234567",
          drivingLicense: "GE123456780",
          handelsregisternummer: "HRB 999999",
          healthInsuranceNumber: "Z000000005",
          identityCardNumber: "T22000129",
          licensePlate: "MIL E 1234",
          lanr: "999999901",
          passportNumber: "C01X00T41",
          postalCode: "10115",
          socialSecurityNumber: "65070803A019",
          taxId: "98765432106",
          taxNumber: "987/654/32100",
          vatId: "DE987654321",
        },
        address: {
          city: "Berlin", district: "Mitte",
          streetName: "Friedrichstraße", buildingNumber: "17",
          unitName: "Main office", timeZone: "Europe/Berlin",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 15,
          annualTurnoverCents: 13000000, averageOrderCents: 2700,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "de", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "de-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-in-branch",
        displayName: "River Workshop Association Bengaluru",
        country: "IN",
        locale: "en-IN",
        currency: "USD",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          aadhaarNumber: "3998-7654-3211",
          gstin: "07PQRST6789K1Z2",
          pan: "ABCPD1234Z",
          passportNumber: "C3590543",
          vehicleRegistration: "DL3CJI0001",
          voterId: "KSD1287349",
        },
        address: {
          city: "Bengaluru", district: "Indiranagar",
          streetName: "Market Road", buildingNumber: "17",
          unitName: "Main office", timeZone: "Asia/Kolkata",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 16,
          annualTurnoverCents: 13000000, averageOrderCents: 2800,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-it-branch",
        displayName: "River Workshop Association Milano",
        country: "IT",
        locale: "it-IT",
        currency: "EUR",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          driverLicense: "U1K711J11M",
          fiscalCode: "AAAAAA00B11C333N",
          identityCardNumber: "AA12345aa",
          passportNumber: "aa7654321",
          vatCode: "01333550_323",
        },
        address: {
          city: "Milano", district: "Brera",
          streetName: "Via Solferino", buildingNumber: "17",
          unitName: "Main office", timeZone: "Europe/Rome",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 17,
          annualTurnoverCents: 13000000, averageOrderCents: 2900,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "it", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "it-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-kr-branch",
        displayName: "River Workshop Association Seoul",
        country: "KR",
        locale: "ko-KR",
        currency: "KRW",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          brn: "104-82-13138",
          driverLicense: "112212345612",
          frn: "9111245678906",
          passportNumber: "M678f3456",
          rrn: "9601211021413",
        },
        address: {
          city: "Seoul", district: "Mapo",
          streetName: "World Cup Road", buildingNumber: "17",
          unitName: "Main office", timeZone: "Asia/Seoul",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 18,
          annualTurnoverCents: 13000000, averageOrderCents: 3000,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "ko", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "ko-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-ng-branch",
        displayName: "River Workshop Association Lagos",
        country: "NG",
        locale: "en-NG",
        currency: "USD",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          nin: "01234567895",
          vehicleRegistration: "ABJ-123XY",
        },
        address: {
          city: "Lagos", district: "Ikeja",
          streetName: "Allen Avenue", buildingNumber: "17",
          unitName: "Main office", timeZone: "Africa/Lagos",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 19,
          annualTurnoverCents: 13000000, averageOrderCents: 3100,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-ph-branch",
        displayName: "River Workshop Association Manila",
        country: "PH",
        locale: "en-PH",
        currency: "USD",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          passportNumber: "eb1234567",
          tin: "000-123-456-000",
          umid: "123456789012",
        },
        address: {
          city: "Manila", district: "Makati",
          streetName: "Ayala Avenue", buildingNumber: "17",
          unitName: "Main office", timeZone: "Asia/Manila",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 20,
          annualTurnoverCents: 13000000, averageOrderCents: 3200,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-pl-branch",
        displayName: "River Workshop Association Warszawa",
        country: "PL",
        locale: "pl-PL",
        currency: "EUR",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          pesel: "11111111116",
        },
        address: {
          city: "Warszawa", district: "Śródmieście",
          streetName: "Marszałkowska", buildingNumber: "17",
          unitName: "Main office", timeZone: "Europe/Warsaw",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 21,
          annualTurnoverCents: 13000000, averageOrderCents: 3300,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "pl", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "pl-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-sg-branch",
        displayName: "River Workshop Association Singapore",
        country: "SG",
        locale: "en-SG",
        currency: "USD",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          nricFin: "A1234567Z",
          uen: "53125226d",
        },
        address: {
          city: "Singapore", district: "Outram",
          streetName: "Neil Road", buildingNumber: "17",
          unitName: "Main office", timeZone: "Asia/Singapore",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 22,
          annualTurnoverCents: 13000000, averageOrderCents: 3400,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-za-branch",
        displayName: "River Workshop Association Cape Town",
        country: "ZA",
        locale: "en-ZA",
        currency: "USD",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          companyRegistrationNumber: "2009/199240/23",
          driverLicense: "40260039Y068",
          idNumber: "0001015002288",
          incomeTaxNumber: "1234567890",
          licensePlate: "GET 103 WP",
          passportNumber: "T99887766",
          trafficRegisterNumber: "6001015000076",
          vatNumber: "4170229407",
        },
        address: {
          city: "Cape Town", district: "Gardens",
          streetName: "Kloof Street", buildingNumber: "17",
          unitName: "Main office", timeZone: "Africa/Johannesburg",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 23,
          annualTurnoverCents: 13000000, averageOrderCents: 3500,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-es-branch",
        displayName: "River Workshop Association Madrid",
        country: "ES",
        locale: "es-ES",
        currency: "EUR",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          nie: "z8078221m",
          nif: "55555555k",
          passportNumber: "XyZ987654",
        },
        address: {
          city: "Madrid", district: "Centro",
          streetName: "Calle Mayor", buildingNumber: "17",
          unitName: "Main office", timeZone: "Europe/Madrid",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 24,
          annualTurnoverCents: 13000000, averageOrderCents: 3600,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "es", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "es-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-se-branch",
        displayName: "River Workshop Association Stockholm",
        country: "SE",
        locale: "sv-SE",
        currency: "EUR",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          organisationsnummer: "2120000142",
          personnummer: "199109242397",
        },
        address: {
          city: "Stockholm", district: "Södermalm",
          streetName: "Götgatan", buildingNumber: "17",
          unitName: "Main office", timeZone: "Europe/Stockholm",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 25,
          annualTurnoverCents: 13000000, averageOrderCents: 3700,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "sv", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "sv-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-th-branch",
        displayName: "River Workshop Association Bangkok",
        country: "TH",
        locale: "th-TH",
        currency: "USD",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          tnin: "1220000000007",
        },
        address: {
          city: "Bangkok", district: "Watthana",
          streetName: "Sukhumvit Road", buildingNumber: "17",
          unitName: "Main office", timeZone: "Asia/Bangkok",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 26,
          annualTurnoverCents: 13000000, averageOrderCents: 3800,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "th", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "th-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-tr-branch",
        displayName: "River Workshop Association Istanbul",
        country: "TR",
        locale: "tr-TR",
        currency: "EUR",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          licensePlate: "34 abc 1234",
          nationalIdNumber: "79059236630",
        },
        address: {
          city: "Istanbul", district: "Kadıköy",
          streetName: "Bahariye Street", buildingNumber: "17",
          unitName: "Main office", timeZone: "Europe/Istanbul",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 27,
          annualTurnoverCents: 13000000, averageOrderCents: 3900,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "tr", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "tr-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-gb-branch",
        displayName: "River Workshop Association London",
        country: "GB",
        locale: "en-GB",
        currency: "GBP",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          drivingLicence: "JONES710153J99EF",
          nhsNumber: "0032698674",
          nino: "AB123456C",
          passportNumber: "XY9876543",
          postcode: "EC1A 1BB",
          vehicleRegistration: "A123 BCD",
        },
        address: {
          city: "London", district: "Camden",
          streetName: "High Street", buildingNumber: "17",
          unitName: "Main office", timeZone: "Europe/London",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 28,
          annualTurnoverCents: 13000000, averageOrderCents: 4000,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "river-workshops-us-branch",
        displayName: "River Workshop Association Seattle",
        country: "US",
        locale: "en-US",
        currency: "USD",
        contactName: "Arun Shah",
        contactRole: "Regional operations manager",
        identifiers: {
          routingNumber: "3222-7162-7",
          deaNumber: "BB1388568",
          bankAccount: "945456787654",
          driverLicense: "H12234567",
          memberId: "AET987654",
          priorAuthorizationNumber: "PA-987654321",
          claimNumber: "123456789012345",
          prescriptionNumber: "7654321",
          referralNumber: "INF2025001234",
          providerTaxId: "20-1234567",
          itinNumber: "911-70-1234",
          mbiNumber: "1eg4-te5-mk73",
          npiNumber: "1234567893",
          passportNumber: "A12803456",
          ssn: "987-65-4322",
        },
        address: {
          city: "Seattle", district: "Fremont",
          streetName: "Fremont Avenue", buildingNumber: "17",
          unitName: "Main office", timeZone: "America/Los_Angeles",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2010, employeeCount: 29,
          annualTurnoverCents: 13000000, averageOrderCents: 4100,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "river-workshops",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
    ],
  };
}

export function buildLighthouseCareScenario(): TenantScenario {
  return {
    tenant: {
      slug: "lighthouse-care",
      displayName: "Lighthouse Care Network",
      defaultCurrency: "USD",
      defaultLocale: "en",
      enabledCountries: ["AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"],
      plan: "standard",
      monthlyDocumentLimit: 5000,
      isExportEnabled: true,
    },
    verification: {
      cardNumber: "3528000700000000",
      bitcoinAddress: "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
      openedAt: "5-May-2021",
      email: "info@presidio.site",
      iban: "AZ21NABZ00000000137010001944",
      ipAddress: "2001:db8::8a2e:370:7334",
      macAddress: "01-23-45-67-89-AB",
      website: "http://microsoft.vlaanderen",
      correlationId: "1ec9414c-232a-6b00-b3c8-9e6bdeced846",
    },
    paymentAmountCents: 18600,
    partialRefundCents: 2100,
    payoutAmountCents: 5000,
    expectedMerchantCount: 18,
    expectedDocumentCount: 81,
    merchants: [
      {
        externalReference: "lighthouse-care-au-branch",
        displayName: "Lighthouse Care Network Sydney",
        country: "AU",
        locale: "en-AU",
        currency: "AUD",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          abn: "51 824 753 556",
          acn: "006249976",
          medicareNumber: "2123 45670 1",
          tfn: "876 543 210",
        },
        address: {
          city: "Sydney", district: "Surry Hills",
          streetName: "Crown Street", buildingNumber: "18",
          unitName: "Main office", timeZone: "Australia/Sydney",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 12,
          annualTurnoverCents: 13100000, averageOrderCents: 2400,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-ca-branch",
        displayName: "Lighthouse Care Network Ottawa",
        country: "CA",
        locale: "en-CA",
        currency: "CAD",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          postalCode: "K1A 1Z1",
          sin: "550090112",
        },
        address: {
          city: "Ottawa", district: "Centretown",
          streetName: "Bank Street", buildingNumber: "18",
          unitName: "Main office", timeZone: "America/Toronto",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 13,
          annualTurnoverCents: 13100000, averageOrderCents: 2500,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-fi-branch",
        displayName: "Lighthouse Care Network Helsinki",
        country: "FI",
        locale: "fi-FI",
        currency: "EUR",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          personalIdentityCode: "040594V9030",
        },
        address: {
          city: "Helsinki", district: "Kallio",
          streetName: "Hämeentie", buildingNumber: "18",
          unitName: "Main office", timeZone: "Europe/Helsinki",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 14,
          annualTurnoverCents: 13100000, averageOrderCents: 2600,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "fi", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "fi-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-de-branch",
        displayName: "Lighthouse Care Network Berlin",
        country: "DE",
        locale: "de-DE",
        currency: "EUR",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          bsnr: "021234568",
          drivingLicense: "MU123456785",
          handelsregisternummer: "HRB 123456",
          healthInsuranceNumber: "Z999999997",
          identityCardNumber: "T00000000",
          licensePlate: "MIL EF 1234E",
          lanr: "123456601",
          passportNumber: "c01234565",
          postalCode: "80331",
          socialSecurityNumber: "20151090B023",
          taxId: "12345678903",
          taxNumber: "12/345/6789",
          vatId: "DE100000001",
        },
        address: {
          city: "Berlin", district: "Mitte",
          streetName: "Friedrichstraße", buildingNumber: "18",
          unitName: "Main office", timeZone: "Europe/Berlin",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 15,
          annualTurnoverCents: 13100000, averageOrderCents: 2700,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "de", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "de-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-in-branch",
        displayName: "Lighthouse Care Network Bengaluru",
        country: "IN",
        locale: "en-IN",
        currency: "USD",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          aadhaarNumber: "312345678909",
          gstin: "01ABCDE1234F1Z5",
          pan: "ABCND1234Z",
          passportNumber: "A3456781",
          vehicleRegistration: "DL01CA1234",
          voterId: "KSD1287349",
        },
        address: {
          city: "Bengaluru", district: "Indiranagar",
          streetName: "Market Road", buildingNumber: "18",
          unitName: "Main office", timeZone: "Asia/Kolkata",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 16,
          annualTurnoverCents: 13100000, averageOrderCents: 2800,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-it-branch",
        displayName: "Lighthouse Care Network Milano",
        country: "IT",
        locale: "it-IT",
        currency: "EUR",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          driverLicense: "AA0123456B",
          fiscalCode: "AAAAAA00B11C333Y",
          identityCardNumber: "1234567Aa",
          passportNumber: "AA1234567",
          vatCode: "01333550323",
        },
        address: {
          city: "Milano", district: "Brera",
          streetName: "Via Solferino", buildingNumber: "18",
          unitName: "Main office", timeZone: "Europe/Rome",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 17,
          annualTurnoverCents: 13100000, averageOrderCents: 2900,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "it", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "it-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-kr-branch",
        displayName: "Lighthouse Care Network Seoul",
        country: "KR",
        locale: "ko-KR",
        currency: "KRW",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          brn: "104-86-56659",
          driverLicense: "13-22-123456-12",
          frn: "050912-6000012",
          passportNumber: "M901g7890",
          rrn: "050912-2000019",
        },
        address: {
          city: "Seoul", district: "Mapo",
          streetName: "World Cup Road", buildingNumber: "18",
          unitName: "Main office", timeZone: "Asia/Seoul",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 18,
          annualTurnoverCents: 13100000, averageOrderCents: 3000,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "ko", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "ko-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-ng-branch",
        displayName: "Lighthouse Care Network Lagos",
        country: "NG",
        locale: "en-NG",
        currency: "USD",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          nin: "12345678902",
          vehicleRegistration: "app-456cv",
        },
        address: {
          city: "Lagos", district: "Ikeja",
          streetName: "Allen Avenue", buildingNumber: "18",
          unitName: "Main office", timeZone: "Africa/Lagos",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 19,
          annualTurnoverCents: 13100000, averageOrderCents: 3100,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-ph-branch",
        displayName: "Lighthouse Care Network Manila",
        country: "PH",
        locale: "en-PH",
        currency: "USD",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          passportNumber: "P1234567A",
          tin: "000123456",
          umid: "987654321098",
        },
        address: {
          city: "Manila", district: "Makati",
          streetName: "Ayala Avenue", buildingNumber: "18",
          unitName: "Main office", timeZone: "Asia/Manila",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 20,
          annualTurnoverCents: 13100000, averageOrderCents: 3200,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-pl-branch",
        displayName: "Lighthouse Care Network Warszawa",
        country: "PL",
        locale: "pl-PL",
        currency: "EUR",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          pesel: "44051401458",
        },
        address: {
          city: "Warszawa", district: "Śródmieście",
          streetName: "Marszałkowska", buildingNumber: "18",
          unitName: "Main office", timeZone: "Europe/Warsaw",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 21,
          annualTurnoverCents: 13100000, averageOrderCents: 3300,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "pl", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "pl-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-sg-branch",
        displayName: "Lighthouse Care Network Singapore",
        country: "SG",
        locale: "en-SG",
        currency: "USD",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          nricFin: "B1234567Z",
          uen: "t16rf0037c",
        },
        address: {
          city: "Singapore", district: "Outram",
          streetName: "Neil Road", buildingNumber: "18",
          unitName: "Main office", timeZone: "Asia/Singapore",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 22,
          annualTurnoverCents: 13100000, averageOrderCents: 3400,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-za-branch",
        displayName: "Lighthouse Care Network Cape Town",
        country: "ZA",
        locale: "en-ZA",
        currency: "USD",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          companyRegistrationNumber: "2014/256030/07",
          driverLicense: "60390002CGBV",
          idNumber: "8001015009087",
          incomeTaxNumber: "9123456789",
          licensePlate: "015 SBZ EC",
          passportNumber: "A34855903",
          trafficRegisterNumber: "1234567890123",
          vatNumber: "4250281542",
        },
        address: {
          city: "Cape Town", district: "Gardens",
          streetName: "Kloof Street", buildingNumber: "18",
          unitName: "Main office", timeZone: "Africa/Johannesburg",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 23,
          annualTurnoverCents: 13100000, averageOrderCents: 3500,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-es-branch",
        displayName: "Lighthouse Care Network Madrid",
        country: "ES",
        locale: "es-ES",
        currency: "EUR",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          nie: "Z8078221M",
          nif: "12345678z",
          passportNumber: "AAA123456",
        },
        address: {
          city: "Madrid", district: "Centro",
          streetName: "Calle Mayor", buildingNumber: "18",
          unitName: "Main office", timeZone: "Europe/Madrid",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 24,
          annualTurnoverCents: 13100000, averageOrderCents: 3600,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "es", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "es-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-se-branch",
        displayName: "Lighthouse Care Network Stockholm",
        country: "SE",
        locale: "sv-SE",
        currency: "EUR",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          organisationsnummer: "556703-7485",
          personnummer: "19910924-2397",
        },
        address: {
          city: "Stockholm", district: "Södermalm",
          streetName: "Götgatan", buildingNumber: "18",
          unitName: "Main office", timeZone: "Europe/Stockholm",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 25,
          annualTurnoverCents: 13100000, averageOrderCents: 3700,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "sv", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "sv-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-th-branch",
        displayName: "Lighthouse Care Network Bangkok",
        country: "TH",
        locale: "th-TH",
        currency: "USD",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          tnin: "1520000000004",
        },
        address: {
          city: "Bangkok", district: "Watthana",
          streetName: "Sukhumvit Road", buildingNumber: "18",
          unitName: "Main office", timeZone: "Asia/Bangkok",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 26,
          annualTurnoverCents: 13100000, averageOrderCents: 3800,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "th", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "th-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-tr-branch",
        displayName: "Lighthouse Care Network Istanbul",
        country: "TR",
        locale: "tr-TR",
        currency: "EUR",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          licensePlate: "01 A 12",
          nationalIdNumber: "64625294480",
        },
        address: {
          city: "Istanbul", district: "Kadıköy",
          streetName: "Bahariye Street", buildingNumber: "18",
          unitName: "Main office", timeZone: "Europe/Istanbul",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 27,
          annualTurnoverCents: 13100000, averageOrderCents: 3900,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "tr", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "tr-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-gb-branch",
        displayName: "Lighthouse Care Network London",
        country: "GB",
        locale: "en-GB",
        currency: "GBP",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          drivingLicence: "SMITH802290AB1CD",
          nhsNumber: "401-023-2137",
          nino: "AB 12 34 56 C",
          passportNumber: "ab1234567",
          postcode: "GIR 0AA",
          vehicleRegistration: "K1 ABC",
        },
        address: {
          city: "London", district: "Camden",
          streetName: "High Street", buildingNumber: "18",
          unitName: "Main office", timeZone: "Europe/London",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 28,
          annualTurnoverCents: 13100000, averageOrderCents: 4000,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "lighthouse-care-us-branch",
        displayName: "Lighthouse Care Network Seattle",
        country: "US",
        locale: "en-US",
        currency: "USD",
        contactName: "Sofia Lind",
        contactRole: "Regional operations manager",
        identifiers: {
          routingNumber: "121042882",
          deaNumber: "K92993548",
          bankAccount: "945456787654",
          driverLicense: "H12234567",
          memberId: "CIGNA123456",
          priorAuthorizationNumber: "pa-123456",
          claimNumber: "CLM456789123",
          prescriptionNumber: "4455667",
          referralNumber: "inf123456",
          providerTaxId: "67-1234567",
          itinNumber: "911-53-1234",
          mbiNumber: "1EG4-TE5-MK73",
          npiNumber: "1245319599",
          passportNumber: "912803456",
          ssn: "987-65-4323",
        },
        address: {
          city: "Seattle", district: "Fremont",
          streetName: "Fremont Avenue", buildingNumber: "18",
          unitName: "Main office", timeZone: "America/Los_Angeles",
        },
        business: {
          sector: "healthcare", legalForm: "partnership",
          foundedYear: 2011, employeeCount: 29,
          annualTurnoverCents: 13100000, averageOrderCents: 4100,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: true,
        },
        tags: ["healthcare", "self_service", "standard"],
        statementDescriptor: "lighthouse-care",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
    ],
  };
}

export function buildSummitMakersScenario(): TenantScenario {
  return {
    tenant: {
      slug: "summit-makers",
      displayName: "Summit Makers Alliance",
      defaultCurrency: "USD",
      defaultLocale: "en",
      enabledCountries: ["AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"],
      plan: "enterprise",
      monthlyDocumentLimit: 5000,
      isExportEnabled: true,
    },
    verification: {
      cardNumber: "6759649826438453",
      bitcoinAddress: "bc1p5d7rjq7g6rdk2yhzks9smlaqtedr4dekq08ge8ztwac72sfr9rusxg3297",
      openedAt: "05/21/21",
      email: "user@xn--80ak6aa92e.com",
      iban: "AZ21 NABZ 0000 0000 1370 1000 1944",
      ipAddress: "2001:db8:85a3::8a2e:370",
      macAddress: "01-b3-4a-67-d9-cf",
      website: "https://webhook.site/a8eedfd6-9d8a-44e0-b0fc-cc7d517db5dc?q=1&b=2",
      correlationId: "018f4f8e-9a3b-7c3d-8e9f-1a2b3c4d5e6f",
    },
    paymentAmountCents: 18700,
    partialRefundCents: 2200,
    payoutAmountCents: 5000,
    expectedMerchantCount: 18,
    expectedDocumentCount: 81,
    merchants: [
      {
        externalReference: "summit-makers-au-branch",
        displayName: "Summit Makers Alliance Sydney",
        country: "AU",
        locale: "en-AU",
        currency: "AUD",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          abn: "51824753556",
          acn: "000000180",
          medicareNumber: "2123456701",
          tfn: "876543210",
        },
        address: {
          city: "Sydney", district: "Surry Hills",
          streetName: "Crown Street", buildingNumber: "19",
          unitName: "Main office", timeZone: "Australia/Sydney",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 12,
          annualTurnoverCents: 13200000, averageOrderCents: 2400,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-ca-branch",
        displayName: "Summit Makers Alliance Ottawa",
        country: "CA",
        locale: "en-CA",
        currency: "CAD",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          postalCode: "K1A 0A1",
          sin: "130-692-544",
        },
        address: {
          city: "Ottawa", district: "Centretown",
          streetName: "Bank Street", buildingNumber: "19",
          unitName: "Main office", timeZone: "America/Toronto",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 13,
          annualTurnoverCents: 13200000, averageOrderCents: 2500,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-fi-branch",
        displayName: "Summit Makers Alliance Helsinki",
        country: "FI",
        locale: "fi-FI",
        currency: "EUR",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          personalIdentityCode: "040594V902Y",
        },
        address: {
          city: "Helsinki", district: "Kallio",
          streetName: "Hämeentie", buildingNumber: "19",
          unitName: "Main office", timeZone: "Europe/Helsinki",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 14,
          annualTurnoverCents: 13200000, averageOrderCents: 2600,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "fi", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "fi-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-de-branch",
        displayName: "Summit Makers Alliance Berlin",
        country: "DE",
        locale: "de-DE",
        currency: "EUR",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          bsnr: "521234567",
          drivingLicense: "BO12345678A",
          handelsregisternummer: "HRB 1",
          healthInsuranceNumber: "a123456780",
          identityCardNumber: "T99999999",
          licensePlate: "B-AB-1234",
          lanr: "234567701",
          passportNumber: "C01234565",
          postalCode: "22085",
          socialSecurityNumber: "38551285K051",
          taxId: "98765432106",
          taxNumber: "12/3456/7890",
          vatId: "DE 136 695 976",
        },
        address: {
          city: "Berlin", district: "Mitte",
          streetName: "Friedrichstraße", buildingNumber: "19",
          unitName: "Main office", timeZone: "Europe/Berlin",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 15,
          annualTurnoverCents: 13200000, averageOrderCents: 2700,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "de", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "de-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-in-branch",
        displayName: "Summit Makers Alliance Bengaluru",
        country: "IN",
        locale: "en-IN",
        currency: "USD",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          aadhaarNumber: "399876543211",
          gstin: "37ABCDE1234F1Z5",
          pan: "A1111DFSFS",
          passportNumber: "B3097651",
          vehicleRegistration: "GJ09AB1234",
          voterId: "KSD1287349",
        },
        address: {
          city: "Bengaluru", district: "Indiranagar",
          streetName: "Market Road", buildingNumber: "19",
          unitName: "Main office", timeZone: "Asia/Kolkata",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 16,
          annualTurnoverCents: 13200000, averageOrderCents: 2800,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-it-branch",
        displayName: "Summit Makers Alliance Milano",
        country: "IT",
        locale: "it-IT",
        currency: "EUR",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          driverLicense: "U1H00B000C",
          fiscalCode: "AAAAAA00B11C333N",
          identityCardNumber: "AA12345aa",
          passportNumber: "aa7654321",
          vatCode: "01333550_323",
        },
        address: {
          city: "Milano", district: "Brera",
          streetName: "Via Solferino", buildingNumber: "19",
          unitName: "Main office", timeZone: "Europe/Rome",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 17,
          annualTurnoverCents: 13200000, averageOrderCents: 2900,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "it", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "it-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-kr-branch",
        displayName: "Summit Makers Alliance Seoul",
        country: "KR",
        locale: "ko-KR",
        currency: "KRW",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          brn: "1048656659",
          driverLicense: "28 22 123456 12",
          frn: "0509126000012",
          passportNumber: "M456B7890",
          rrn: "0509122000019",
        },
        address: {
          city: "Seoul", district: "Mapo",
          streetName: "World Cup Road", buildingNumber: "19",
          unitName: "Main office", timeZone: "Asia/Seoul",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 18,
          annualTurnoverCents: 13200000, averageOrderCents: 3000,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "ko", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "ko-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-ng-branch",
        displayName: "Summit Makers Alliance Lagos",
        country: "NG",
        locale: "en-NG",
        currency: "USD",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          nin: "98765432102",
          vehicleRegistration: "APP-456CV",
        },
        address: {
          city: "Lagos", district: "Ikeja",
          streetName: "Allen Avenue", buildingNumber: "19",
          unitName: "Main office", timeZone: "Africa/Lagos",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 19,
          annualTurnoverCents: 13200000, averageOrderCents: 3100,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-ph-branch",
        displayName: "Summit Makers Alliance Manila",
        country: "PH",
        locale: "en-PH",
        currency: "USD",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          passportNumber: "Z0000000Z",
          tin: "000123456000",
          umid: "0111-1234567-8",
        },
        address: {
          city: "Manila", district: "Makati",
          streetName: "Ayala Avenue", buildingNumber: "19",
          unitName: "Main office", timeZone: "Asia/Manila",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 20,
          annualTurnoverCents: 13200000, averageOrderCents: 3200,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-pl-branch",
        displayName: "Summit Makers Alliance Warszawa",
        country: "PL",
        locale: "pl-PL",
        currency: "EUR",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          pesel: "02070803628",
        },
        address: {
          city: "Warszawa", district: "Śródmieście",
          streetName: "Marszałkowska", buildingNumber: "19",
          unitName: "Main office", timeZone: "Europe/Warsaw",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 21,
          annualTurnoverCents: 13200000, averageOrderCents: 3300,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "pl", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "pl-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-sg-branch",
        displayName: "Summit Makers Alliance Singapore",
        country: "SG",
        locale: "en-SG",
        currency: "USD",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          nricFin: "S2740116C",
          uen: "53125226D",
        },
        address: {
          city: "Singapore", district: "Outram",
          streetName: "Neil Road", buildingNumber: "19",
          unitName: "Main office", timeZone: "Asia/Singapore",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 22,
          annualTurnoverCents: 13200000, averageOrderCents: 3400,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-za-branch",
        displayName: "Summit Makers Alliance Cape Town",
        country: "ZA",
        locale: "en-ZA",
        currency: "USD",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          companyRegistrationNumber: "2020/804826/07",
          driverLicense: "4024048D4P60",
          idNumber: "8001015000086",
          incomeTaxNumber: "2987654321",
          licensePlate: "MT77GJGP",
          passportNumber: "D12345678",
          trafficRegisterNumber: "6001015000076",
          vatNumber: "4100168758",
        },
        address: {
          city: "Cape Town", district: "Gardens",
          streetName: "Kloof Street", buildingNumber: "19",
          unitName: "Main office", timeZone: "Africa/Johannesburg",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 23,
          annualTurnoverCents: 13200000, averageOrderCents: 3500,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-es-branch",
        displayName: "Summit Makers Alliance Madrid",
        country: "ES",
        locale: "es-ES",
        currency: "EUR",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          nie: "X9613851N",
          nif: "12345678Z",
          passportNumber: "XYZ987654",
        },
        address: {
          city: "Madrid", district: "Centro",
          streetName: "Calle Mayor", buildingNumber: "19",
          unitName: "Main office", timeZone: "Europe/Madrid",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 24,
          annualTurnoverCents: 13200000, averageOrderCents: 3600,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "es", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "es-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-se-branch",
        displayName: "Summit Makers Alliance Stockholm",
        country: "SE",
        locale: "sv-SE",
        currency: "EUR",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          organisationsnummer: "5567037485",
          personnummer: "199201232387",
        },
        address: {
          city: "Stockholm", district: "Södermalm",
          streetName: "Götgatan", buildingNumber: "19",
          unitName: "Main office", timeZone: "Europe/Stockholm",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 25,
          annualTurnoverCents: 13200000, averageOrderCents: 3700,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "sv", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "sv-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-th-branch",
        displayName: "Summit Makers Alliance Bangkok",
        country: "TH",
        locale: "th-TH",
        currency: "USD",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          tnin: "1580000000004",
        },
        address: {
          city: "Bangkok", district: "Watthana",
          streetName: "Sukhumvit Road", buildingNumber: "19",
          unitName: "Main office", timeZone: "Asia/Bangkok",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 26,
          annualTurnoverCents: 13200000, averageOrderCents: 3800,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "th", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "th-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-tr-branch",
        displayName: "Summit Makers Alliance Istanbul",
        country: "TR",
        locale: "tr-TR",
        currency: "EUR",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          licensePlate: "81 A 12",
          nationalIdNumber: "10000000146",
        },
        address: {
          city: "Istanbul", district: "Kadıköy",
          streetName: "Bahariye Street", buildingNumber: "19",
          unitName: "Main office", timeZone: "Europe/Istanbul",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 27,
          annualTurnoverCents: 13200000, averageOrderCents: 3900,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "tr", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "tr-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-gb-branch",
        displayName: "Summit Makers Alliance London",
        country: "GB",
        locale: "en-GB",
        currency: "GBP",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          drivingLicence: "SMITH812310AB1CD",
          nhsNumber: "221 395 1837",
          nino: "AA 12 34 56 B",
          passportNumber: "CD7654321",
          postcode: "M11AA",
          vehicleRegistration: "M456DEF",
        },
        address: {
          city: "London", district: "Camden",
          streetName: "High Street", buildingNumber: "19",
          unitName: "Main office", timeZone: "Europe/London",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 28,
          annualTurnoverCents: 13200000, averageOrderCents: 4000,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "summit-makers-us-branch",
        displayName: "Summit Makers Alliance Seattle",
        country: "US",
        locale: "en-US",
        currency: "USD",
        contactName: "Deniz Kaya",
        contactRole: "Regional operations manager",
        identifiers: {
          routingNumber: "0711-0130-7",
          deaNumber: "BB1388568",
          bankAccount: "945456787654",
          driverLicense: "H12234567",
          memberId: "K123456789",
          priorAuthorizationNumber: "PA-123456",
          claimNumber: "clm123456",
          prescriptionNumber: "RX789456123",
          referralNumber: "REF123456",
          providerTaxId: "99-1234567",
          itinNumber: "911-64-1234",
          mbiNumber: "1EG4TE5MK73",
          npiNumber: "1003000126",
          passportNumber: "Z12803456",
          ssn: "987-65-4324",
        },
        address: {
          city: "Seattle", district: "Fremont",
          streetName: "Fremont Avenue", buildingNumber: "19",
          unitName: "Main office", timeZone: "America/Los_Angeles",
        },
        business: {
          sector: "manufacturing", legalForm: "company",
          foundedYear: 2012, employeeCount: 29,
          annualTurnoverCents: 13200000, averageOrderCents: 4100,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["manufacturing", "partner", "priority"],
        statementDescriptor: "summit-makers",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
    ],
  };
}

export function buildMeadowCommerceScenario(): TenantScenario {
  return {
    tenant: {
      slug: "meadow-commerce",
      displayName: "Meadow Commerce Partners",
      defaultCurrency: "USD",
      defaultLocale: "en",
      enabledCountries: ["AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"],
      plan: "standard",
      monthlyDocumentLimit: 5000,
      isExportEnabled: true,
    },
    verification: {
      cardNumber: "4111111111111111",
      bitcoinAddress: "16Yeky6GMjeNkAiNcBY7ZhrLoMSgg1BoyZ",
      openedAt: "5/21/21",
      email: "a@xn--d1acufc.xn--p1ai",
      iban: "BH67BMAG00001299123456",
      ipAddress: "2001:db8::1",
      macAddress: "0d-B3-4a-6A-d9-cF",
      website: "https://www.microsoft.com/store/abc/",
      correlationId: "550e8400-e29b-81d4-a716-446655440000",
    },
    paymentAmountCents: 18800,
    partialRefundCents: 2300,
    payoutAmountCents: 5000,
    expectedMerchantCount: 18,
    expectedDocumentCount: 81,
    merchants: [
      {
        externalReference: "meadow-commerce-au-branch",
        displayName: "Meadow Commerce Partners Sydney",
        country: "AU",
        locale: "en-AU",
        currency: "AUD",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          abn: "51 824 753 556",
          acn: "000 000 019",
          medicareNumber: "2123 45670 1",
          tfn: "876 543 210",
        },
        address: {
          city: "Sydney", district: "Surry Hills",
          streetName: "Crown Street", buildingNumber: "20",
          unitName: "Main office", timeZone: "Australia/Sydney",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 12,
          annualTurnoverCents: 13300000, averageOrderCents: 2400,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-ca-branch",
        displayName: "Meadow Commerce Partners Ottawa",
        country: "CA",
        locale: "en-CA",
        currency: "CAD",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          postalCode: "K1A0A1",
          sin: "258 933 688",
        },
        address: {
          city: "Ottawa", district: "Centretown",
          streetName: "Bank Street", buildingNumber: "20",
          unitName: "Main office", timeZone: "America/Toronto",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 13,
          annualTurnoverCents: 13300000, averageOrderCents: 2500,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-fi-branch",
        displayName: "Meadow Commerce Partners Helsinki",
        country: "FI",
        locale: "fi-FI",
        currency: "EUR",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          personalIdentityCode: "050594U903M",
        },
        address: {
          city: "Helsinki", district: "Kallio",
          streetName: "Hämeentie", buildingNumber: "20",
          unitName: "Main office", timeZone: "Europe/Helsinki",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 14,
          annualTurnoverCents: 13300000, averageOrderCents: 2600,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "fi", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "fi-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-de-branch",
        displayName: "Meadow Commerce Partners Berlin",
        country: "DE",
        locale: "de-DE",
        currency: "EUR",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          bsnr: "711234567",
          drivingLicense: "MU12345678B",
          handelsregisternummer: "HRB123456",
          healthInsuranceNumber: "A000500015",
          identityCardNumber: "t22000129",
          licensePlate: "M-XY-999",
          lanr: "100000601",
          passportNumber: "F12345671",
          postalCode: "01001",
          socialSecurityNumber: "15070649C103",
          taxId: "12345678903",
          taxNumber: "123/3456/7890",
          vatId: "DE 129 273 398",
        },
        address: {
          city: "Berlin", district: "Mitte",
          streetName: "Friedrichstraße", buildingNumber: "20",
          unitName: "Main office", timeZone: "Europe/Berlin",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 15,
          annualTurnoverCents: 13300000, averageOrderCents: 2700,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "de", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "de-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-in-branch",
        displayName: "Meadow Commerce Partners Bengaluru",
        country: "IN",
        locale: "en-IN",
        currency: "USD",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          aadhaarNumber: "3123 4567 8909",
          gstin: "27ABCDE1234F1Z5",
          pan: "AAASA1111R",
          passportNumber: "C3590543",
          vehicleRegistration: "KA53ME3456",
          voterId: "KSD1287349",
        },
        address: {
          city: "Bengaluru", district: "Indiranagar",
          streetName: "Market Road", buildingNumber: "20",
          unitName: "Main office", timeZone: "Asia/Kolkata",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 16,
          annualTurnoverCents: 13300000, averageOrderCents: 2800,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-it-branch",
        displayName: "Meadow Commerce Partners Milano",
        country: "IT",
        locale: "it-IT",
        currency: "EUR",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          driverLicense: "U1K711J11M",
          fiscalCode: "AAAAAA00B11C333Y",
          identityCardNumber: "1234567Aa",
          passportNumber: "AA1234567",
          vatCode: "01333550323",
        },
        address: {
          city: "Milano", district: "Brera",
          streetName: "Via Solferino", buildingNumber: "20",
          unitName: "Main office", timeZone: "Europe/Rome",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 17,
          annualTurnoverCents: 13300000, averageOrderCents: 2900,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "it", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "it-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-kr-branch",
        displayName: "Meadow Commerce Partners Seoul",
        country: "KR",
        locale: "ko-KR",
        currency: "KRW",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          brn: "104-82-13138",
          driverLicense: "11-22-123456-12",
          frn: "911124-5678901",
          passportNumber: "M789C1234",
          rrn: "960121-1234567",
        },
        address: {
          city: "Seoul", district: "Mapo",
          streetName: "World Cup Road", buildingNumber: "20",
          unitName: "Main office", timeZone: "Asia/Seoul",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 18,
          annualTurnoverCents: 13300000, averageOrderCents: 3000,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "ko", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "ko-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-ng-branch",
        displayName: "Meadow Commerce Partners Lagos",
        country: "NG",
        locale: "en-NG",
        currency: "USD",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          nin: "01234567895",
          vehicleRegistration: "ABJ-001AA",
        },
        address: {
          city: "Lagos", district: "Ikeja",
          streetName: "Allen Avenue", buildingNumber: "20",
          unitName: "Main office", timeZone: "Africa/Lagos",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 19,
          annualTurnoverCents: 13300000, averageOrderCents: 3100,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-ph-branch",
        displayName: "Meadow Commerce Partners Manila",
        country: "PH",
        locale: "en-PH",
        currency: "USD",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          passportNumber: "EB1234567",
          tin: "000-123-456-001",
          umid: "0000-0000000-0",
        },
        address: {
          city: "Manila", district: "Makati",
          streetName: "Ayala Avenue", buildingNumber: "20",
          unitName: "Main office", timeZone: "Asia/Manila",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 20,
          annualTurnoverCents: 13300000, averageOrderCents: 3200,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-pl-branch",
        displayName: "Meadow Commerce Partners Warszawa",
        country: "PL",
        locale: "pl-PL",
        currency: "EUR",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          pesel: "11111111116",
        },
        address: {
          city: "Warszawa", district: "Śródmieście",
          streetName: "Marszałkowska", buildingNumber: "20",
          unitName: "Main office", timeZone: "Europe/Warsaw",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 21,
          annualTurnoverCents: 13300000, averageOrderCents: 3300,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "pl", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "pl-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-sg-branch",
        displayName: "Meadow Commerce Partners Singapore",
        country: "SG",
        locale: "en-SG",
        currency: "USD",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          nricFin: "T1234567Z",
          uen: "201434292D",
        },
        address: {
          city: "Singapore", district: "Outram",
          streetName: "Neil Road", buildingNumber: "20",
          unitName: "Main office", timeZone: "Asia/Singapore",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 22,
          annualTurnoverCents: 13300000, averageOrderCents: 3400,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-za-branch",
        displayName: "Meadow Commerce Partners Cape Town",
        country: "ZA",
        locale: "en-ZA",
        currency: "USD",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          companyRegistrationNumber: "CK2001/123456",
          driverLicense: "30040008X6Z6",
          idNumber: "9202201234088",
          incomeTaxNumber: "0123456789",
          licensePlate: "KD93GKGP",
          passportNumber: "M87654321",
          trafficRegisterNumber: "1234567890123",
          vatNumber: "4020269678",
        },
        address: {
          city: "Cape Town", district: "Gardens",
          streetName: "Kloof Street", buildingNumber: "20",
          unitName: "Main office", timeZone: "Africa/Johannesburg",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 23,
          annualTurnoverCents: 13300000, averageOrderCents: 3500,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-es-branch",
        displayName: "Meadow Commerce Partners Madrid",
        country: "ES",
        locale: "es-ES",
        currency: "EUR",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          nie: "Y8063915Z",
          nif: "55555555K",
          passportNumber: "aaa123456",
        },
        address: {
          city: "Madrid", district: "Centro",
          streetName: "Calle Mayor", buildingNumber: "20",
          unitName: "Main office", timeZone: "Europe/Madrid",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 24,
          annualTurnoverCents: 13300000, averageOrderCents: 3600,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "es", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "es-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-se-branch",
        displayName: "Meadow Commerce Partners Stockholm",
        country: "SE",
        locale: "sv-SE",
        currency: "EUR",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          organisationsnummer: "212000-0142",
          personnummer: "9201232387",
        },
        address: {
          city: "Stockholm", district: "Södermalm",
          streetName: "Götgatan", buildingNumber: "20",
          unitName: "Main office", timeZone: "Europe/Stockholm",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 25,
          annualTurnoverCents: 13300000, averageOrderCents: 3700,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "sv", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "sv-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-th-branch",
        displayName: "Meadow Commerce Partners Bangkok",
        country: "TH",
        locale: "th-TH",
        currency: "USD",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          tnin: "1234567890121",
        },
        address: {
          city: "Bangkok", district: "Watthana",
          streetName: "Sukhumvit Road", buildingNumber: "20",
          unitName: "Main office", timeZone: "Asia/Bangkok",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 26,
          annualTurnoverCents: 13300000, averageOrderCents: 3800,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "th", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "th-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-tr-branch",
        displayName: "Meadow Commerce Partners Istanbul",
        country: "TR",
        locale: "tr-TR",
        currency: "EUR",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          licensePlate: "07 AB 123",
          nationalIdNumber: "76543210794",
        },
        address: {
          city: "Istanbul", district: "Kadıköy",
          streetName: "Bahariye Street", buildingNumber: "20",
          unitName: "Main office", timeZone: "Europe/Istanbul",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 27,
          annualTurnoverCents: 13300000, averageOrderCents: 3900,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "tr", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "tr-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-gb-branch",
        displayName: "Meadow Commerce Partners London",
        country: "GB",
        locale: "en-GB",
        currency: "GBP",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          drivingLicence: "SMITH851010AB1CD",
          nhsNumber: "0032698674",
          nino: "hh 01 02 03 d",
          passportNumber: "AB1234567",
          postcode: "EC1A1BB",
          vehicleRegistration: "ABC 123D",
        },
        address: {
          city: "London", district: "Camden",
          streetName: "High Street", buildingNumber: "20",
          unitName: "Main office", timeZone: "Europe/London",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 28,
          annualTurnoverCents: 13300000, averageOrderCents: 4000,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
      {
        externalReference: "meadow-commerce-us-branch",
        displayName: "Meadow Commerce Partners Seattle",
        country: "US",
        locale: "en-US",
        currency: "USD",
        contactName: "Jiho Park",
        contactRole: "Regional operations manager",
        identifiers: {
          routingNumber: "121000358",
          deaNumber: "K92993548",
          bankAccount: "945456787654",
          driverLicense: "H12234567",
          memberId: "abc123456",
          priorAuthorizationNumber: "PA-123456789012",
          claimNumber: "CLM123456",
          prescriptionNumber: "rX123456",
          referralNumber: "INF123456789012",
          providerTaxId: "12-3456789",
          itinNumber: "911701234",
          mbiNumber: "9XX9-XX9-XX99",
          npiNumber: "1234-567-893",
          passportNumber: "A12803456",
          ssn: "987-65-4325",
        },
        address: {
          city: "Seattle", district: "Fremont",
          streetName: "Fremont Avenue", buildingNumber: "20",
          unitName: "Main office", timeZone: "America/Los_Angeles",
        },
        business: {
          sector: "retail", legalForm: "partnership",
          foundedYear: 2013, employeeCount: 29,
          annualTurnoverCents: 13300000, averageOrderCents: 4100,
          hasPhysicalStore: true, hasOnlineStore: true,
        },
        preferences: {
          language: "en", invoiceDelivery: "email",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["retail", "self_service", "standard"],
        statementDescriptor: "meadow-commerce",
        supportQueue: "en-merchant-support",
        onboardingChannel: "self_service",
      },
    ],
  };
}

export function buildHorizonProfessionalsScenario(): TenantScenario {
  return {
    tenant: {
      slug: "horizon-professionals",
      displayName: "Horizon Professional Services",
      defaultCurrency: "USD",
      defaultLocale: "en",
      enabledCountries: ["AU", "CA", "FI", "DE", "IN", "IT", "KR", "NG", "PH", "PL", "SG", "ZA", "ES", "SE", "TH", "TR", "GB", "US"],
      plan: "enterprise",
      monthlyDocumentLimit: 5000,
      isExportEnabled: true,
    },
    verification: {
      cardNumber: "4917300800000000",
      bitcoinAddress: "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy",
      openedAt: "21/05/21",
      email: "info@presidio.site",
      iban: "BH67 BMAG 0000 1299 1234 56",
      ipAddress: "fe80::1%eth0",
      macAddress: "0012.3456.789A",
      website: "microsoft.com",
      correlationId: "550E8400-E29B-41D4-A716-446655440000",
    },
    paymentAmountCents: 18900,
    partialRefundCents: 2400,
    payoutAmountCents: 5000,
    expectedMerchantCount: 18,
    expectedDocumentCount: 81,
    merchants: [
      {
        externalReference: "horizon-professionals-au-branch",
        displayName: "Horizon Professional Services Sydney",
        country: "AU",
        locale: "en-AU",
        currency: "AUD",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          abn: "51824753556",
          acn: "005 499 981",
          medicareNumber: "2123456701",
          tfn: "876543210",
        },
        address: {
          city: "Sydney", district: "Surry Hills",
          streetName: "Crown Street", buildingNumber: "21",
          unitName: "Main office", timeZone: "Australia/Sydney",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2014, employeeCount: 12,
          annualTurnoverCents: 13400000, averageOrderCents: 2400,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-ca-branch",
        displayName: "Horizon Professional Services Ottawa",
        country: "CA",
        locale: "en-CA",
        currency: "CAD",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          postalCode: "k1a 0a1",
          sin: "130 692 544",
        },
        address: {
          city: "Ottawa", district: "Centretown",
          streetName: "Bank Street", buildingNumber: "21",
          unitName: "Main office", timeZone: "America/Toronto",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2014, employeeCount: 13,
          annualTurnoverCents: 13400000, averageOrderCents: 2500,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-fi-branch",
        displayName: "Horizon Professional Services Helsinki",
        country: "FI",
        locale: "fi-FI",
        currency: "EUR",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          personalIdentityCode: "050594U902L",
        },
        address: {
          city: "Helsinki", district: "Kallio",
          streetName: "Hämeentie", buildingNumber: "21",
          unitName: "Main office", timeZone: "Europe/Helsinki",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2014, employeeCount: 14,
          annualTurnoverCents: 13400000, averageOrderCents: 2600,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "fi", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "fi-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-de-branch",
        displayName: "Horizon Professional Services Berlin",
        country: "DE",
        locale: "de-DE",
        currency: "EUR",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          bsnr: "351234567",
          drivingLicense: "HH98765432C",
          handelsregisternummer: "HRA 12345",
          healthInsuranceNumber: "C000500021",
          identityCardNumber: "L01X00T44",
          licensePlate: "HH-AB-1234",
          lanr: "987654401",
          passportNumber: "L01X00T44",
          postalCode: "99998",
          socialSecurityNumber: "65070803A019",
          taxId: "98765432106",
          taxNumber: "0281508150123",
          vatId: "DE 136695976",
        },
        address: {
          city: "Berlin", district: "Mitte",
          streetName: "Friedrichstraße", buildingNumber: "21",
          unitName: "Main office", timeZone: "Europe/Berlin",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2014, employeeCount: 15,
          annualTurnoverCents: 13400000, averageOrderCents: 2700,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "de", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "de-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-in-branch",
        displayName: "Horizon Professional Services Bengaluru",
        country: "IN",
        locale: "en-IN",
        currency: "USD",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          aadhaarNumber: "3998 7654 3211",
          gstin: "07PQRST6789K1Z2",
          pan: "ABCPD1234Z",
          passportNumber: "A3456781",
          vehicleRegistration: "KA99ME3456",
          voterId: "KSD1287349",
        },
        address: {
          city: "Bengaluru", district: "Indiranagar",
          streetName: "Market Road", buildingNumber: "21",
          unitName: "Main office", timeZone: "Asia/Kolkata",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2014, employeeCount: 16,
          annualTurnoverCents: 13400000, averageOrderCents: 2800,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-it-branch",
        displayName: "Horizon Professional Services Milano",
        country: "IT",
        locale: "it-IT",
        currency: "EUR",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          driverLicense: "AA0123456B",
          fiscalCode: "AAAAAA00B11C333N",
          identityCardNumber: "AA12345aa",
          passportNumber: "aa7654321",
          vatCode: "01333550_323",
        },
        address: {
          city: "Milano", district: "Brera",
          streetName: "Via Solferino", buildingNumber: "21",
          unitName: "Main office", timeZone: "Europe/Rome",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2014, employeeCount: 17,
          annualTurnoverCents: 13400000, averageOrderCents: 2900,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "it", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "it-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-kr-branch",
        displayName: "Horizon Professional Services Seoul",
        country: "KR",
        locale: "ko-KR",
        currency: "KRW",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          brn: "104-86-56659",
          driverLicense: "112212345612",
          frn: "9111245678901",
          passportNumber: "M12345678",
          rrn: "9601211234567",
        },
        address: {
          city: "Seoul", district: "Mapo",
          streetName: "World Cup Road", buildingNumber: "21",
          unitName: "Main office", timeZone: "Asia/Seoul",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2014, employeeCount: 18,
          annualTurnoverCents: 13400000, averageOrderCents: 3000,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "ko", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "ko-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-ng-branch",
        displayName: "Horizon Professional Services Lagos",
        country: "NG",
        locale: "en-NG",
        currency: "USD",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          nin: "12345678902",
          vehicleRegistration: "KJA-999PZ",
        },
        address: {
          city: "Lagos", district: "Ikeja",
          streetName: "Allen Avenue", buildingNumber: "21",
          unitName: "Main office", timeZone: "Africa/Lagos",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2014, employeeCount: 19,
          annualTurnoverCents: 13400000, averageOrderCents: 3100,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "digest", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true, canIssueRefunds: true,
          canRequestPayouts: true, shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-ph-branch",
        displayName: "Horizon Professional Services Manila",
        country: "PH",
        locale: "en-PH",
        currency: "USD",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          passportNumber: "AA0000000",
          tin: "000-123-456",
          umid: "001112345678",
        },
        address: {
          city: "Manila",
          district: "Makati",
          streetName: "Ayala Avenue",
          buildingNumber: "21",
          unitName: "Main office",
          timeZone: "Asia/Manila",
        },
        business: {
          sector: "services", legalForm: "company",
          foundedYear: 2014, employeeCount: 20,
          annualTurnoverCents: 13400000, averageOrderCents: 3200,
          hasPhysicalStore: true, hasOnlineStore: false,
        },
        preferences: {
          language: "en", invoiceDelivery: "portal",
          notificationMode: "immediate", hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000, dailyPaymentCents: 250000,
          refundWindowDays: 30, settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true,
          canIssueRefunds: true,
          canRequestPayouts: true,
          shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-pl-branch",
        displayName: "Horizon Professional Services Warszawa",
        country: "PL",
        locale: "pl-PL",
        currency: "EUR",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          pesel: "44051401458",
        },
        address: {
          city: "Warszawa",
          district: "Śródmieście",
          streetName: "Marszałkowska",
          buildingNumber: "21",
          unitName: "Main office",
          timeZone: "Europe/Warsaw",
        },
        business: {
          sector: "services",
          legalForm: "company",
          foundedYear: 2014,
          employeeCount: 21,
          annualTurnoverCents: 13400000,
          averageOrderCents: 3300,
          hasPhysicalStore: true,
          hasOnlineStore: false,
        },
        preferences: {
          language: "pl",
          invoiceDelivery: "portal",
          notificationMode: "digest",
          hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000,
          dailyPaymentCents: 250000,
          refundWindowDays: 30,
          settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true,
          canIssueRefunds: true,
          canRequestPayouts: true,
          shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "pl-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-sg-branch",
        displayName: "Horizon Professional Services Singapore",
        country: "SG",
        locale: "en-SG",
        currency: "USD",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          nricFin: "F2346401L",
          uen: "T16RF0037C",
        },
        address: {
          city: "Singapore",
          district: "Outram",
          streetName: "Neil Road",
          buildingNumber: "21",
          unitName: "Main office",
          timeZone: "Asia/Singapore",
        },
        business: {
          sector: "services",
          legalForm: "company",
          foundedYear: 2014,
          employeeCount: 22,
          annualTurnoverCents: 13400000,
          averageOrderCents: 3400,
          hasPhysicalStore: true,
          hasOnlineStore: false,
        },
        preferences: {
          language: "en",
          invoiceDelivery: "portal",
          notificationMode: "immediate",
          hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000,
          dailyPaymentCents: 250000,
          refundWindowDays: 30,
          settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true,
          canIssueRefunds: true,
          canRequestPayouts: true,
          shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-za-branch",
        displayName: "Horizon Professional Services Cape Town",
        country: "ZA",
        locale: "en-ZA",
        currency: "USD",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          companyRegistrationNumber: "CK1998/654321",
          driverLicense: "4046048YPC9T",
          idNumber: "0002294321191",
          incomeTaxNumber: "1234567890",
          licensePlate: "PMG017GP",
          passportNumber: "T11223344",
          trafficRegisterNumber: "6001015000076",
          vatNumber: "4170229407",
        },
        address: {
          city: "Cape Town",
          district: "Gardens",
          streetName: "Kloof Street",
          buildingNumber: "21",
          unitName: "Main office",
          timeZone: "Africa/Johannesburg",
        },
        business: {
          sector: "services",
          legalForm: "company",
          foundedYear: 2014,
          employeeCount: 23,
          annualTurnoverCents: 13400000,
          averageOrderCents: 3500,
          hasPhysicalStore: true,
          hasOnlineStore: false,
        },
        preferences: {
          language: "en",
          invoiceDelivery: "portal",
          notificationMode: "digest",
          hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000,
          dailyPaymentCents: 250000,
          refundWindowDays: 30,
          settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true,
          canIssueRefunds: true,
          canRequestPayouts: true,
          shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-es-branch",
        displayName: "Horizon Professional Services Madrid",
        country: "ES",
        locale: "es-ES",
        currency: "EUR",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          nie: "Y8063915-Z",
          nif: "55555555-K",
          passportNumber: "xyz987654",
        },
        address: {
          city: "Madrid",
          district: "Centro",
          streetName: "Calle Mayor",
          buildingNumber: "21",
          unitName: "Main office",
          timeZone: "Europe/Madrid",
        },
        business: {
          sector: "services",
          legalForm: "company",
          foundedYear: 2014,
          employeeCount: 24,
          annualTurnoverCents: 13400000,
          averageOrderCents: 3600,
          hasPhysicalStore: true,
          hasOnlineStore: false,
        },
        preferences: {
          language: "es",
          invoiceDelivery: "portal",
          notificationMode: "immediate",
          hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000,
          dailyPaymentCents: 250000,
          refundWindowDays: 30,
          settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true,
          canIssueRefunds: true,
          canRequestPayouts: true,
          shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "es-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-se-branch",
        displayName: "Horizon Professional Services Stockholm",
        country: "SE",
        locale: "sv-SE",
        currency: "EUR",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          organisationsnummer: "2120000142",
          personnummer: "200109022392",
        },
        address: {
          city: "Stockholm",
          district: "Södermalm",
          streetName: "Götgatan",
          buildingNumber: "21",
          unitName: "Main office",
          timeZone: "Europe/Stockholm",
        },
        business: {
          sector: "services",
          legalForm: "company",
          foundedYear: 2014,
          employeeCount: 25,
          annualTurnoverCents: 13400000,
          averageOrderCents: 3700,
          hasPhysicalStore: true,
          hasOnlineStore: false,
        },
        preferences: {
          language: "sv",
          invoiceDelivery: "portal",
          notificationMode: "digest",
          hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000,
          dailyPaymentCents: 250000,
          refundWindowDays: 30,
          settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true,
          canIssueRefunds: true,
          canRequestPayouts: true,
          shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "sv-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-th-branch",
        displayName: "Horizon Professional Services Bangkok",
        country: "TH",
        locale: "th-TH",
        currency: "USD",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          tnin: "2345678901234",
        },
        address: {
          city: "Bangkok",
          district: "Watthana",
          streetName: "Sukhumvit Road",
          buildingNumber: "21",
          unitName: "Main office",
          timeZone: "Asia/Bangkok",
        },
        business: {
          sector: "services",
          legalForm: "company",
          foundedYear: 2014,
          employeeCount: 26,
          annualTurnoverCents: 13400000,
          averageOrderCents: 3800,
          hasPhysicalStore: true,
          hasOnlineStore: false,
        },
        preferences: {
          language: "th",
          invoiceDelivery: "portal",
          notificationMode: "immediate",
          hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000,
          dailyPaymentCents: 250000,
          refundWindowDays: 30,
          settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true,
          canIssueRefunds: true,
          canRequestPayouts: true,
          shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "th-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-tr-branch",
        displayName: "Horizon Professional Services Istanbul",
        country: "TR",
        locale: "tr-TR",
        currency: "EUR",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          licensePlate: "34 ABC 1234",
          nationalIdNumber: "36493665440",
        },
        address: {
          city: "Istanbul",
          district: "Kadıköy",
          streetName: "Bahariye Street",
          buildingNumber: "21",
          unitName: "Main office",
          timeZone: "Europe/Istanbul",
        },
        business: {
          sector: "services",
          legalForm: "company",
          foundedYear: 2014,
          employeeCount: 27,
          annualTurnoverCents: 13400000,
          averageOrderCents: 3900,
          hasPhysicalStore: true,
          hasOnlineStore: false,
        },
        preferences: {
          language: "tr",
          invoiceDelivery: "portal",
          notificationMode: "digest",
          hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000,
          dailyPaymentCents: 250000,
          refundWindowDays: 30,
          settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true,
          canIssueRefunds: true,
          canRequestPayouts: true,
          shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "tr-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-gb-branch",
        displayName: "Horizon Professional Services London",
        country: "GB",
        locale: "en-GB",
        currency: "GBP",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          drivingLicence: "SMITH862310AB1CD",
          nhsNumber: "401-023-2137",
          nino: "tw987654a",
          passportNumber: "XY9876543",
          postcode: "DN551PT",
          vehicleRegistration: "ABC 1D",
        },
        address: {
          city: "London",
          district: "Camden",
          streetName: "High Street",
          buildingNumber: "21",
          unitName: "Main office",
          timeZone: "Europe/London",
        },
        business: {
          sector: "services",
          legalForm: "company",
          foundedYear: 2014,
          employeeCount: 28,
          annualTurnoverCents: 13400000,
          averageOrderCents: 4000,
          hasPhysicalStore: true,
          hasOnlineStore: false,
        },
        preferences: {
          language: "en",
          invoiceDelivery: "portal",
          notificationMode: "immediate",
          hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000,
          dailyPaymentCents: 250000,
          refundWindowDays: 30,
          settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true,
          canIssueRefunds: true,
          canRequestPayouts: true,
          shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
      {
        externalReference: "horizon-professionals-us-branch",
        displayName: "Horizon Professional Services Seattle",
        country: "US",
        locale: "en-US",
        currency: "USD",
        contactName: "Mina Santos",
        contactRole: "Regional operations manager",
        identifiers: {
          routingNumber: "3222-7162-7",
          deaNumber: "BB1388568",
          bankAccount: "945456787654",
          driverLicense: "H12234567",
          memberId: "AbC123456",
          priorAuthorizationNumber: "987654321",
          claimNumber: "CLM123456789012345",
          prescriptionNumber: "RX123456",
          referralNumber: "2025001234",
          providerTaxId: "20-1234567",
          itinNumber: "911-70-1234",
          mbiNumber: "3CD5-FG7-HJ89",
          npiNumber: "1234 567 893",
          passportNumber: "912803456",
          ssn: "987-65-4326",
        },
        address: {
          city: "Seattle",
          district: "Fremont",
          streetName: "Fremont Avenue",
          buildingNumber: "21",
          unitName: "Main office",
          timeZone: "America/Los_Angeles",
        },
        business: {
          sector: "services",
          legalForm: "company",
          foundedYear: 2014,
          employeeCount: 29,
          annualTurnoverCents: 13400000,
          averageOrderCents: 4100,
          hasPhysicalStore: true,
          hasOnlineStore: false,
        },
        preferences: {
          language: "en",
          invoiceDelivery: "portal",
          notificationMode: "digest",
          hasMarketingConsent: false,
          hasServiceConsent: true,
        },
        limits: {
          singlePaymentCents: 50000,
          dailyPaymentCents: 250000,
          refundWindowDays: 30,
          settlementDelayDays: 2,
        },
        capabilities: {
          canAcceptPayments: true,
          canIssueRefunds: true,
          canRequestPayouts: true,
          shouldRequireSecondReviewer: false,
        },
        tags: ["services", "partner", "priority"],
        statementDescriptor: "horizon-professional",
        supportQueue: "en-merchant-support",
        onboardingChannel: "partner",
      },
    ],
  };
}

export const TENANT_SCENARIOS: ReadonlyArray<TenantScenario> = [
  buildAtlasMarketScenario(),
  buildHarborServicesScenario(),
  buildCedarClinicsScenario(),
  buildNorthstarSupplyScenario(),
  buildOrchardStoresScenario(),
  buildRiverWorkshopsScenario(),
  buildLighthouseCareScenario(),
  buildSummitMakersScenario(),
  buildMeadowCommerceScenario(),
  buildHorizonProfessionalsScenario(),
];
