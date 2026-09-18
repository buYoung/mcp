// Run the actual page's business model against values parsed from its HTML forms.
// This checks the model, not browser DOM rendering or event delivery.
import {readFileSync} from "node:fs";
import {runInNewContext} from "node:vm";
import assert from "node:assert/strict";

const [scriptPath, formsPath] = process.argv.slice(2);
const source = readFileSync(scriptPath, "utf8");
const forms = JSON.parse(readFileSync(formsPath, "utf8"));
let bootstrapRegistrations = 0;
const Model = runInNewContext(source + "\nMerchantStore;", {
  structuredClone,
  document: {
    addEventListener(event, callback) {
      assert.equal(event, "DOMContentLoaded");
      assert.equal(typeof callback, "function");
      bootstrapRegistrations++;
    },
  },
});
assert.equal(bootstrapRegistrations, 1);
const store = new Model();
let assertions = 1;
let documents = 0;
function check(condition, message) {
  assertions++;
  assert.ok(condition, message);
}
function rejects(operation, message) {
  let isRejected = false;
  try { operation(); } catch { isRejected = true; }
  check(isRejected, message);
}

for (const form of forms.filter(form => form.kind === "verification")) {
  store.verify(form.tenant, form.fields);
  for (const [field, value] of Object.entries(form.fields)) {
    check(store.verifications.get(form.tenant)[field] === value, "verification value changed");
  }
}
for (const form of forms.filter(form => form.kind === "merchant-form")) {
  const input = {...form.fields, identifiers: form.identifiers};
  const reference = input.externalReference;
  store.register(input);
  rejects(() => store.register(input), "duplicate registration accepted");
  for (const [field, value] of Object.entries(input.identifiers)) {
    check(store.merchant(reference).input.identifiers[field] === value, "stored form value changed");
    documents++;
  }
  // Mutation of the submitted form must not mutate the persisted copy.
  const firstField = Object.keys(input.identifiers)[0];
  const originalValue = input.identifiers[firstField];
  input.identifiers[firstField] = "edited draft";
  check(store.merchant(reference).input.identifiers[firstField] === originalValue, "request object retained by reference");
  input.identifiers[firstField] = originalValue;
  rejects(() => store.authorize(reference, 18000), "draft accepted payment");
  store.submit(reference);
  rejects(() => store.submit(reference), "duplicate submission accepted");
  store.approve(reference, "reviewer-a");
  if (input.shouldRequireSecondReviewer) {
    check(store.merchant(reference).status === "review", "second review bypassed");
    rejects(() => store.approve(reference, "reviewer-a"), "same reviewer accepted twice");
    store.approve(reference, "reviewer-b");
  }
  check(store.merchant(reference).status === "active", "merchant not active");
  store.authorize(reference, 18000);
  store.authorize(reference, 18000);
  rejects(() => store.authorize(reference, 18001), "conflicting retry accepted");
  store.capture(reference);
  store.capture(reference);
  check(store.balance(reference) === 18000, "capture duplicated");
  store.refund(reference, 1500);
  store.payout(reference, 5000);
  check(store.balance(reference) === 11500, "incorrect balance");
  rejects(() => store.payout(reference, 11501), "overdraft accepted");
  check(store.balance(reference) === 11500, "failed payout changed balance");
  rejects(() => store.refund(reference, 18000), "excess refund accepted");
}
check(store.merchants.size === 180 && documents === 810, "missing documents");
check(store.ledger.reduce((sum, entry) => sum + entry.amountCents, 0) === 0, "ledger not balanced");
const exported = store.exportDocuments();
check(exported.split("\n").length === documents + 1, "export row count changed");
for (const merchant of store.merchants.values()) {
  for (const value of Object.values(merchant.input.identifiers)) {
    check(exported.includes(value), "export changed document");
  }
}
console.log(JSON.stringify({scenarios: store.verifications.size, merchants: store.merchants.size,
  documents, assertions, validation: "HTML structure, JavaScript syntax and business model; no browser DOM"}));
